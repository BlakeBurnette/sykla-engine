/**
 * Sykla BLE (Web Bluetooth) unified manager.
 * Handles all BLE cycling peripherals:
 *   - Smart Trainer (FTMS 0x1826) — bidirectional: read data + write resistance
 *   - Heart Rate Monitor (0x180D) — read HR + R-R intervals
 *   - Cycling Power Meter (0x1818) — read power + cadence
 *   - Speed & Cadence Sensor (0x1816) — read wheel/crank revolutions
 *
 * Exposes window.syklaBle namespace for Rust/WASM interop.
 */

window.syklaBle = (() => {
    // ---- BLE Service & Characteristic UUIDs ----
    const SERVICE = {
        HEART_RATE: 0x180D,
        CYCLING_POWER: 0x1818,
        CYCLING_SPEED_CADENCE: 0x1816,
        FTMS: 0x1826,
    };
    const CHAR = {
        HR_MEASUREMENT: 0x2A37,
        CSC_MEASUREMENT: 0x2A5B,
        POWER_MEASUREMENT: 0x2A63,
        FTMS_FEATURE: 0x2ACC,
        INDOOR_BIKE_DATA: 0x2AD2,
        FTMS_CONTROL_POINT: 0x2AD9,
        FTMS_STATUS: 0x2ADA,
    };

    // ---- State ----
    const devices = new Map(); // deviceId -> { device, server, type, services }
    let controlPointChar = null;
    let trainerControlled = false;

    // Latest data per device type
    let trainerData = null;  // { speed_kmh, cadence_rpm, power_watts, heart_rate_bpm }
    let hrmData = null;      // { heart_rate_bpm, rr_intervals, contact }
    let powerData = null;    // { power_watts, cadence_rpm, speed_mps }
    let cscData = null;      // { speed_kmh, cadence_rpm }

    // Connection flags
    let trainerConnected = false;
    let hrmConnected = false;
    let powerConnected = false;
    let cscConnected = false;

    // Delta tracking for cumulative counters
    let _lastCrankRevs = null;
    let _lastCrankTime = null;
    let _lastWheelRevs = null;
    let _lastWheelTime = null;

    const WHEEL_CIRCUMFERENCE_M = 2.105; // 700x25c

    // ==================================================================
    //  Device Discovery
    // ==================================================================

    async function scanForTrainer() {
        try {
            const device = await navigator.bluetooth.requestDevice({
                filters: [{ services: [SERVICE.FTMS] }],
                optionalServices: [SERVICE.CYCLING_POWER, SERVICE.CYCLING_SPEED_CADENCE],
            });
            return await _connectDevice(device, 'trainer');
        } catch (e) {
            console.error('[Sykla BLE] Trainer scan failed:', e);
            return null;
        }
    }

    async function scanForHeartRateMonitor() {
        try {
            const device = await navigator.bluetooth.requestDevice({
                filters: [{ services: [SERVICE.HEART_RATE] }],
            });
            return await _connectDevice(device, 'hrm');
        } catch (e) {
            console.error('[Sykla BLE] HRM scan failed:', e);
            return null;
        }
    }

    async function scanForPowerMeter() {
        try {
            const device = await navigator.bluetooth.requestDevice({
                filters: [{ services: [SERVICE.CYCLING_POWER] }],
                optionalServices: [SERVICE.CYCLING_SPEED_CADENCE],
            });
            return await _connectDevice(device, 'power_meter');
        } catch (e) {
            console.error('[Sykla BLE] Power meter scan failed:', e);
            return null;
        }
    }

    async function scanForSpeedCadence() {
        try {
            const device = await navigator.bluetooth.requestDevice({
                filters: [{ services: [SERVICE.CYCLING_SPEED_CADENCE] }],
            });
            return await _connectDevice(device, 'speed_cadence');
        } catch (e) {
            console.error('[Sykla BLE] Speed/cadence scan failed:', e);
            return null;
        }
    }

    // ==================================================================
    //  Connection
    // ==================================================================

    async function _connectDevice(device, type) {
        console.log(`[Sykla BLE] Connecting to ${device.name} (${type})...`);

        device.addEventListener('gattserverdisconnected', () => {
            console.warn(`[Sykla BLE] ${device.name} disconnected`);
            devices.delete(device.id);
            _setConnected(type, false);
            _updateUI(type, false);
            _attemptReconnect(device, type);
        });

        const server = await device.gatt.connect();
        const conn = { device, server, type };
        devices.set(device.id, conn);

        switch (type) {
            case 'trainer':
                await _setupFTMS(conn);
                // Also try power service as fallback data source
                try { await _setupCyclingPower(conn); } catch (_) { /* optional */ }
                break;
            case 'hrm':
                await _setupHeartRate(conn);
                break;
            case 'power_meter':
                await _setupCyclingPower(conn);
                break;
            case 'speed_cadence':
                await _setupSpeedCadence(conn);
                break;
        }

        _setConnected(type, true);
        _updateUI(type, true, device.name);
        console.log(`[Sykla BLE] Connected to ${device.name} (${type})`);
        return { id: device.id, name: device.name, type };
    }

    // ==================================================================
    //  Reconnection
    // ==================================================================

    async function _attemptReconnect(device, type, maxAttempts = 5) {
        for (let i = 0; i < maxAttempts; i++) {
            await new Promise(r => setTimeout(r, 1000 * (i + 1))); // exponential-ish backoff
            try {
                if (!device.gatt.connected) {
                    await _connectDevice(device, type);
                    console.log(`[Sykla BLE] Reconnected to ${device.name}`);
                    return;
                }
            } catch (_) {
                console.warn(`[Sykla BLE] Reconnect attempt ${i + 1} failed for ${device.name}`);
            }
        }
        console.error(`[Sykla BLE] Failed to reconnect to ${device.name} after ${maxAttempts} attempts`);
    }

    // ==================================================================
    //  Heart Rate Service (0x180D)
    // ==================================================================

    async function _setupHeartRate(conn) {
        const service = await conn.server.getPrimaryService(SERVICE.HEART_RATE);
        const char = await service.getCharacteristic(CHAR.HR_MEASUREMENT);

        char.addEventListener('characteristicvaluechanged', (event) => {
            hrmData = _parseHeartRate(event.target.value);
        });
        await char.startNotifications();
    }

    function _parseHeartRate(dv) {
        let offset = 0;
        const flags = dv.getUint8(offset++);
        const isUint16 = flags & 0x01;
        const contactDetected = (flags & 0x06) === 0x06;
        const energyPresent = flags & 0x08;
        const rrPresent = flags & 0x10;

        let heartRate;
        if (isUint16) {
            heartRate = dv.getUint16(offset, true);
            offset += 2;
        } else {
            heartRate = dv.getUint8(offset);
            offset += 1;
        }

        if (energyPresent) offset += 2;

        const rrIntervals = [];
        if (rrPresent) {
            while (offset + 1 < dv.byteLength) {
                const rr = dv.getUint16(offset, true);
                rrIntervals.push(rr / 1024.0 * 1000.0); // ms
                offset += 2;
            }
        }

        return { heart_rate_bpm: heartRate, rr_intervals: rrIntervals, contact: contactDetected };
    }

    // ==================================================================
    //  FTMS — Fitness Machine Service (0x1826)
    // ==================================================================

    async function _setupFTMS(conn) {
        const service = await conn.server.getPrimaryService(SERVICE.FTMS);

        // Read feature support
        try {
            const featureChar = await service.getCharacteristic(CHAR.FTMS_FEATURE);
            const val = await featureChar.readValue();
            console.log('[Sykla BLE] FTMS features:', val.getUint32(0, true).toString(16));
        } catch (_) { /* continue */ }

        // Subscribe to Indoor Bike Data
        const bikeDataChar = await service.getCharacteristic(CHAR.INDOOR_BIKE_DATA);
        bikeDataChar.addEventListener('characteristicvaluechanged', (event) => {
            trainerData = _parseIndoorBikeData(event.target.value);
        });
        await bikeDataChar.startNotifications();

        // Subscribe to status (optional)
        try {
            const statusChar = await service.getCharacteristic(CHAR.FTMS_STATUS);
            statusChar.addEventListener('characteristicvaluechanged', (event) => {
                console.log('[Sykla BLE] FTMS status:', event.target.value.getUint8(0));
            });
            await statusChar.startNotifications();
        } catch (_) { /* optional */ }

        // Get Control Point for writing commands
        controlPointChar = await service.getCharacteristic(CHAR.FTMS_CONTROL_POINT);

        // Subscribe to Control Point indications (responses)
        controlPointChar.addEventListener('characteristicvaluechanged', (event) => {
            const dv = event.target.value;
            if (dv.byteLength >= 3) {
                const result = dv.getUint8(2); // 0x01 = success
                if (result !== 0x01) {
                    console.warn('[Sykla BLE] FTMS control point error, result:', result);
                }
            }
        });
        await controlPointChar.startNotifications();

        // Request control of the trainer
        await _requestTrainerControl();
    }

    function _parseIndoorBikeData(dv) {
        const flags = dv.getUint16(0, true);
        let offset = 2;
        const result = { speed_kmh: 0, cadence_rpm: 0, power_watts: 0, heart_rate_bpm: null };

        // Instantaneous Speed (bit 0 NOT set = present)
        if ((flags & 0x01) === 0 && offset + 2 <= dv.byteLength) {
            result.speed_kmh = dv.getUint16(offset, true) * 0.01;
            offset += 2;
        }
        // Average Speed (bit 1)
        if (flags & 0x02) offset += 2;
        // Instantaneous Cadence (bit 2)
        if ((flags & 0x04) && offset + 2 <= dv.byteLength) {
            result.cadence_rpm = dv.getUint16(offset, true) * 0.5;
            offset += 2;
        }
        // Average Cadence (bit 3)
        if (flags & 0x08) offset += 2;
        // Total Distance (bit 4) — 3 bytes
        if (flags & 0x10) offset += 3;
        // Resistance Level (bit 5)
        if (flags & 0x20) offset += 2;
        // Instantaneous Power (bit 6)
        if ((flags & 0x40) && offset + 2 <= dv.byteLength) {
            result.power_watts = dv.getInt16(offset, true);
            offset += 2;
        }
        // Average Power (bit 7)
        if (flags & 0x80) offset += 2;
        // Expended Energy (bit 8) — 5 bytes (total=2, per_hour=2, per_min=1)
        if (flags & 0x100) offset += 5;
        // Heart Rate (bit 9)
        if ((flags & 0x200) && offset + 1 <= dv.byteLength) {
            result.heart_rate_bpm = dv.getUint8(offset);
            offset += 1;
        }

        return result;
    }

    // ==================================================================
    //  Trainer Control (FTMS Control Point 0x2AD9)
    // ==================================================================

    async function _requestTrainerControl() {
        if (!controlPointChar) return false;
        try {
            // Request Control
            await controlPointChar.writeValueWithResponse(new Uint8Array([0x00]));
            trainerControlled = true;
            // Start
            await controlPointChar.writeValueWithResponse(new Uint8Array([0x07]));
            console.log('[Sykla BLE] Trainer control acquired');
            return true;
        } catch (e) {
            console.error('[Sykla BLE] Failed to request trainer control:', e);
            return false;
        }
    }

    /**
     * Set simulation parameters — called at ~1Hz by the game loop.
     * @param {number} windSpeed — m/s (positive = headwind)
     * @param {number} grade — percent grade (e.g. 7.5 for 7.5% climb)
     * @param {number} crr — rolling resistance coefficient (default 0.004)
     * @param {number} cda — wind resistance coefficient kg/m (default 0.39)
     */
    function setSimulationParameters(windSpeed, grade, crr, cda) {
        if (!controlPointChar || !trainerControlled) return;
        // Clamp grade to FTMS range (-16% to +16% for most trainers)
        grade = Math.max(-16, Math.min(16, grade));
        const buf = new ArrayBuffer(7);
        const view = new DataView(buf);
        view.setUint8(0, 0x11);
        view.setInt16(1, Math.round(windSpeed * 1000), true);
        view.setInt16(3, Math.round(grade * 100), true);
        view.setUint8(5, Math.round((crr || 0.004) * 10000));
        view.setUint8(6, Math.round((cda || 0.39) * 100));
        controlPointChar.writeValueWithResponse(new Uint8Array(buf)).catch(e => {
            console.debug('[Sykla BLE] Sim params write skipped:', e.message);
        });
    }

    /**
     * Set target power (ERG mode) — for structured workouts.
     * @param {number} watts — target power in watts
     */
    function setTargetPower(watts) {
        if (!controlPointChar || !trainerControlled) return;
        const buf = new ArrayBuffer(3);
        const view = new DataView(buf);
        view.setUint8(0, 0x05);
        view.setInt16(1, Math.round(watts), true);
        controlPointChar.writeValueWithResponse(new Uint8Array(buf)).catch(e => {
            console.debug('[Sykla BLE] Target power write skipped:', e.message);
        });
    }

    /**
     * Set resistance level directly (0-100%).
     * @param {number} level — 0.0 to 1.0
     */
    function setResistanceLevel(level) {
        if (!controlPointChar || !trainerControlled) return;
        const buf = new ArrayBuffer(2);
        const view = new DataView(buf);
        view.setUint8(0, 0x04);
        view.setUint8(1, Math.round(level * 100));
        controlPointChar.writeValueWithResponse(new Uint8Array(buf)).catch(e => {
            console.debug('[Sykla BLE] Resistance write skipped:', e.message);
        });
    }

    // ==================================================================
    //  Cycling Power Service (0x1818)
    // ==================================================================

    async function _setupCyclingPower(conn) {
        const service = await conn.server.getPrimaryService(SERVICE.CYCLING_POWER);
        const char = await service.getCharacteristic(CHAR.POWER_MEASUREMENT);

        char.addEventListener('characteristicvaluechanged', (event) => {
            const data = _parseCyclingPower(event.target.value);
            // For dedicated power meters, store in powerData.
            // For trainers, trainerData from FTMS takes priority.
            if (conn.type === 'power_meter') {
                powerData = data;
            }
        });
        await char.startNotifications();
    }

    function _parseCyclingPower(dv) {
        let offset = 0;
        const flags = dv.getUint16(offset, true);
        offset += 2;
        const power = dv.getInt16(offset, true);
        offset += 2;
        const result = { power_watts: power };

        // Skip pedal balance if present (bit 0)
        if (flags & 0x01) offset += 1;
        // Skip accumulated torque (bit 2)
        if (flags & 0x04) offset += 2;

        // Wheel revolution data (bit 4)
        if (flags & 0x10) {
            const wheelRevs = dv.getUint32(offset, true); offset += 4;
            const wheelTime = dv.getUint16(offset, true); offset += 2;
            if (_lastWheelRevs !== null) {
                const deltaRevs = wheelRevs - _lastWheelRevs;
                let deltaTime = wheelTime - _lastWheelTime;
                if (deltaTime < 0) deltaTime += 65536;
                if (deltaRevs > 0 && deltaTime > 0) {
                    result.speed_mps = (deltaRevs * WHEEL_CIRCUMFERENCE_M) / (deltaTime / 1024.0);
                }
            }
            _lastWheelRevs = wheelRevs;
            _lastWheelTime = wheelTime;
        }

        // Crank revolution data (bit 5)
        if (flags & 0x20) {
            const crankRevs = dv.getUint16(offset, true); offset += 2;
            const crankTime = dv.getUint16(offset, true); offset += 2;
            if (_lastCrankRevs !== null) {
                let deltaRevs = crankRevs - _lastCrankRevs;
                let deltaTime = crankTime - _lastCrankTime;
                if (deltaTime < 0) deltaTime += 65536;
                if (deltaRevs < 0) deltaRevs += 65536;
                if (deltaRevs > 0 && deltaTime > 0) {
                    result.cadence_rpm = (deltaRevs / (deltaTime / 1024.0)) * 60.0;
                }
            }
            _lastCrankRevs = crankRevs;
            _lastCrankTime = crankTime;
        }

        return result;
    }

    // ==================================================================
    //  Cycling Speed & Cadence Service (0x1816)
    // ==================================================================

    async function _setupSpeedCadence(conn) {
        const service = await conn.server.getPrimaryService(SERVICE.CYCLING_SPEED_CADENCE);
        const char = await service.getCharacteristic(CHAR.CSC_MEASUREMENT);

        char.addEventListener('characteristicvaluechanged', (event) => {
            cscData = _parseCSC(event.target.value);
        });
        await char.startNotifications();
    }

    function _parseCSC(dv) {
        let offset = 0;
        const flags = dv.getUint8(offset++);
        const result = {};

        // Wheel revolution data (bit 0)
        if (flags & 0x01) {
            const wheelRevs = dv.getUint32(offset, true); offset += 4;
            const wheelTime = dv.getUint16(offset, true); offset += 2;
            if (_lastWheelRevs !== null && wheelRevs !== _lastWheelRevs) {
                const deltaRevs = wheelRevs - _lastWheelRevs;
                let deltaTime = wheelTime - _lastWheelTime;
                if (deltaTime < 0) deltaTime += 65536;
                if (deltaTime > 0) {
                    result.speed_kmh = (deltaRevs * WHEEL_CIRCUMFERENCE_M) / (deltaTime / 1024.0) * 3.6;
                }
            }
            _lastWheelRevs = wheelRevs;
            _lastWheelTime = wheelTime;
        }

        // Crank revolution data (bit 1)
        if (flags & 0x02) {
            const crankRevs = dv.getUint16(offset, true); offset += 2;
            const crankTime = dv.getUint16(offset, true); offset += 2;
            if (_lastCrankRevs !== null && crankRevs !== _lastCrankRevs) {
                let deltaRevs = crankRevs - _lastCrankRevs;
                let deltaTime = crankTime - _lastCrankTime;
                if (deltaTime < 0) deltaTime += 65536;
                if (deltaRevs < 0) deltaRevs += 65536;
                if (deltaTime > 0) {
                    result.cadence_rpm = (deltaRevs / (deltaTime / 1024.0)) * 60.0;
                }
            }
            _lastCrankRevs = crankRevs;
            _lastCrankTime = crankTime;
        }

        return result;
    }

    // ==================================================================
    //  Data Access (polled by Rust each frame)
    // ==================================================================

    /**
     * Returns unified JSON with all device data and connection status.
     * Applies data priority resolution:
     *   Power: dedicated power meter > FTMS > CPS broadcast
     *   Cadence: FTMS > CPS > standalone CSC
     *   Speed: FTMS > CSC
     *   Heart rate: dedicated HRM > trainer-bridged HR
     */
    function getLatestData() {
        // Resolve power: dedicated PM > trainer FTMS
        let power = null;
        if (powerData && powerData.power_watts != null) {
            power = powerData.power_watts;
        } else if (trainerData && trainerData.power_watts != null) {
            power = trainerData.power_watts;
        }

        // Resolve cadence: trainer FTMS > power meter CPS > CSC
        let cadence = null;
        if (trainerData && trainerData.cadence_rpm != null) {
            cadence = trainerData.cadence_rpm;
        } else if (powerData && powerData.cadence_rpm != null) {
            cadence = powerData.cadence_rpm;
        } else if (cscData && cscData.cadence_rpm != null) {
            cadence = cscData.cadence_rpm;
        }

        // Resolve speed: trainer FTMS > CSC
        let speed = null;
        if (trainerData && trainerData.speed_kmh != null) {
            speed = trainerData.speed_kmh;
        } else if (cscData && cscData.speed_kmh != null) {
            speed = cscData.speed_kmh;
        } else if (powerData && powerData.speed_mps != null) {
            speed = powerData.speed_mps * 3.6;
        }

        // Resolve heart rate: dedicated HRM > trainer-bridged
        let hr = null;
        if (hrmData && hrmData.heart_rate_bpm != null) {
            hr = hrmData.heart_rate_bpm;
        } else if (trainerData && trainerData.heart_rate_bpm != null) {
            hr = trainerData.heart_rate_bpm;
        }

        return JSON.stringify({
            speed_kmh: speed || 0,
            power_watts: power || 0,
            cadence_rpm: cadence || 0,
            heart_rate_bpm: hr,
            trainer_connected: trainerConnected,
            hrm_connected: hrmConnected,
            power_connected: powerConnected,
            csc_connected: cscConnected,
        });
    }

    /** Legacy compat — returns true if ANY device is connected */
    function isConnected() {
        return trainerConnected || hrmConnected || powerConnected || cscConnected;
    }

    /** Returns true if a trainer is connected */
    function isTrainerConnected() { return trainerConnected; }
    function isHRMConnected() { return hrmConnected; }
    function isPowerConnected() { return powerConnected; }
    function isCSCConnected() { return cscConnected; }

    // ==================================================================
    //  Disconnect
    // ==================================================================

    function disconnect(deviceId) {
        const conn = devices.get(deviceId);
        if (conn && conn.device.gatt.connected) {
            conn.device.gatt.disconnect();
        }
        devices.delete(deviceId);
    }

    function disconnectAll() {
        for (const [id, conn] of devices) {
            if (conn.device.gatt.connected) {
                conn.device.gatt.disconnect();
            }
        }
        devices.clear();
        controlPointChar = null;
        trainerControlled = false;
        trainerData = null;
        hrmData = null;
        powerData = null;
        cscData = null;
        trainerConnected = false;
        hrmConnected = false;
        powerConnected = false;
        cscConnected = false;
    }

    // ==================================================================
    //  Helpers
    // ==================================================================

    function _setConnected(type, connected) {
        switch (type) {
            case 'trainer': trainerConnected = connected; if (!connected) { trainerData = null; controlPointChar = null; trainerControlled = false; } break;
            case 'hrm': hrmConnected = connected; if (!connected) hrmData = null; break;
            case 'power_meter': powerConnected = connected; if (!connected) powerData = null; break;
            case 'speed_cadence': cscConnected = connected; if (!connected) cscData = null; break;
        }
    }

    function _updateUI(type, connected, deviceName) {
        const el = document.getElementById(`ble-${type}`);
        if (!el) return;
        const btn = el.querySelector('button');
        const status = el.querySelector('.ble-status');
        if (connected) {
            el.classList.add('connected');
            if (btn) btn.textContent = 'Disconnect';
            if (status) status.textContent = deviceName || 'Connected';
        } else {
            el.classList.remove('connected');
            if (btn) btn.textContent = 'Pair';
            if (status) status.textContent = 'Not connected';
        }
    }

    // Legacy: connectTrainer() for old index.html compat
    async function connectTrainer() { return await scanForTrainer(); }

    // Expose writeSimParams for raw byte writes (legacy ble.rs compat)
    function writeSimParams(data) {
        if (!controlPointChar || !trainerControlled) return;
        controlPointChar.writeValueWithResponse(new Uint8Array(data)).catch(e => {
            console.warn('[Sykla BLE] Write failed:', e);
        });
    }

    return {
        // Discovery
        scanForTrainer,
        scanForHeartRateMonitor,
        scanForPowerMeter,
        scanForSpeedCadence,
        // Legacy alias
        connectTrainer,
        // Data access
        getLatestData,
        isConnected,
        isTrainerConnected,
        isHRMConnected,
        isPowerConnected,
        isCSCConnected,
        // Trainer commands
        setSimulationParameters,
        setTargetPower,
        setResistanceLevel,
        writeSimParams,
        // Disconnect
        disconnect,
        disconnectAll,
        // Legacy alias
        disconnectTrainer: disconnectAll,
    };
})();

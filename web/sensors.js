/**
 * Sykla Outdoor BLE Sensors — separate from ble.js (which is FTMS for indoor trainers).
 *
 * Outdoor sensors use standard BLE services:
 * - Heart Rate Service: UUID 0x180D, characteristic 0x2A37
 * - Cycling Speed & Cadence: UUID 0x1816, characteristic 0x2A5B
 * - Cycling Power: UUID 0x1818, characteristic 0x2A63
 *
 * Exposes `window.syklaSensors` namespace.
 */

window.syklaSensors = (() => {
    // Heart Rate Monitor
    const HR_SERVICE = 0x180D;
    const HR_MEASUREMENT = 0x2A37;

    // Cycling Speed & Cadence
    const CSC_SERVICE = 0x1816;
    const CSC_MEASUREMENT = 0x2A5B;

    // Cycling Power
    const POWER_SERVICE = 0x1818;
    const POWER_MEASUREMENT = 0x2A63;

    let hrmDevice = null;
    let cscDevice = null;
    let powerDevice = null;

    let latestHR = null;
    let latestCadence = null;
    let latestSpeed = null;
    let latestPower = null;

    // CSC state for delta computation
    let lastWheelRevs = null;
    let lastWheelTime = null;
    let lastCrankRevs = null;
    let lastCrankTime = null;

    // Wheel circumference in meters (700x25c default)
    const WHEEL_CIRCUMFERENCE_M = 2.105;

    // ------------------------------------------------------------------
    // Heart Rate Monitor
    // ------------------------------------------------------------------

    async function connectHRM() {
        try {
            hrmDevice = await navigator.bluetooth.requestDevice({
                filters: [{ services: [HR_SERVICE] }],
            });

            hrmDevice.addEventListener('gattserverdisconnected', () => {
                latestHR = null;
                console.log('[Sykla Sensors] HRM disconnected');
                updateSensorStatus('hrm', false);
            });

            const server = await hrmDevice.gatt.connect();
            const service = await server.getPrimaryService(HR_SERVICE);
            const char = await service.getCharacteristic(HR_MEASUREMENT);
            await char.startNotifications();

            char.addEventListener('characteristicvaluechanged', (event) => {
                const value = event.target.value;
                // Heart rate measurement format:
                // Bit 0 of flags: 0 = UINT8 HR, 1 = UINT16 HR
                const flags = value.getUint8(0);
                if (flags & 0x01) {
                    latestHR = value.getUint16(1, true);
                } else {
                    latestHR = value.getUint8(1);
                }
            });

            console.log('[Sykla Sensors] HRM connected:', hrmDevice.name);
            updateSensorStatus('hrm', true);
            return true;
        } catch (err) {
            console.error('[Sykla Sensors] HRM connection failed:', err);
            return false;
        }
    }

    // ------------------------------------------------------------------
    // Cycling Speed & Cadence
    // ------------------------------------------------------------------

    async function connectSpeedCadence() {
        try {
            cscDevice = await navigator.bluetooth.requestDevice({
                filters: [{ services: [CSC_SERVICE] }],
            });

            cscDevice.addEventListener('gattserverdisconnected', () => {
                latestCadence = null;
                latestSpeed = null;
                console.log('[Sykla Sensors] CSC disconnected');
                updateSensorStatus('csc', false);
            });

            const server = await cscDevice.gatt.connect();
            const service = await server.getPrimaryService(CSC_SERVICE);
            const char = await service.getCharacteristic(CSC_MEASUREMENT);
            await char.startNotifications();

            char.addEventListener('characteristicvaluechanged', (event) => {
                const value = event.target.value;
                const flags = value.getUint8(0);
                let offset = 1;

                // Wheel revolution data present (bit 0)
                if (flags & 0x01) {
                    const wheelRevs = value.getUint32(offset, true);
                    offset += 4;
                    const wheelTime = value.getUint16(offset, true); // 1/1024s
                    offset += 2;

                    if (lastWheelRevs !== null && wheelRevs !== lastWheelRevs) {
                        const revDelta = wheelRevs - lastWheelRevs;
                        let timeDelta = wheelTime - lastWheelTime;
                        if (timeDelta < 0) timeDelta += 65536; // uint16 wrap
                        const timeSecs = timeDelta / 1024;
                        if (timeSecs > 0) {
                            const distM = revDelta * WHEEL_CIRCUMFERENCE_M;
                            latestSpeed = (distM / timeSecs) * 3.6; // m/s to km/h
                        }
                    }
                    lastWheelRevs = wheelRevs;
                    lastWheelTime = wheelTime;
                }

                // Crank revolution data present (bit 1)
                if (flags & 0x02) {
                    const crankRevs = value.getUint16(offset, true);
                    offset += 2;
                    const crankTime = value.getUint16(offset, true); // 1/1024s
                    offset += 2;

                    if (lastCrankRevs !== null && crankRevs !== lastCrankRevs) {
                        const revDelta = crankRevs - lastCrankRevs;
                        let timeDelta = crankTime - lastCrankTime;
                        if (timeDelta < 0) timeDelta += 65536;
                        const timeSecs = timeDelta / 1024;
                        if (timeSecs > 0) {
                            latestCadence = (revDelta / timeSecs) * 60; // RPM
                        }
                    }
                    lastCrankRevs = crankRevs;
                    lastCrankTime = crankTime;
                }
            });

            console.log('[Sykla Sensors] CSC connected:', cscDevice.name);
            updateSensorStatus('csc', true);
            return true;
        } catch (err) {
            console.error('[Sykla Sensors] CSC connection failed:', err);
            return false;
        }
    }

    // ------------------------------------------------------------------
    // Cycling Power Meter
    // ------------------------------------------------------------------

    async function connectPowerMeter() {
        try {
            powerDevice = await navigator.bluetooth.requestDevice({
                filters: [{ services: [POWER_SERVICE] }],
            });

            powerDevice.addEventListener('gattserverdisconnected', () => {
                latestPower = null;
                console.log('[Sykla Sensors] Power meter disconnected');
                updateSensorStatus('power', false);
            });

            const server = await powerDevice.gatt.connect();
            const service = await server.getPrimaryService(POWER_SERVICE);
            const char = await service.getCharacteristic(POWER_MEASUREMENT);
            await char.startNotifications();

            char.addEventListener('characteristicvaluechanged', (event) => {
                const value = event.target.value;
                // Cycling Power Measurement:
                // Bytes 0-1: flags (uint16)
                // Bytes 2-3: instantaneous power (sint16, watts)
                if (value.byteLength >= 4) {
                    latestPower = value.getInt16(2, true);
                }
            });

            console.log('[Sykla Sensors] Power meter connected:', powerDevice.name);
            updateSensorStatus('power', true);
            return true;
        } catch (err) {
            console.error('[Sykla Sensors] Power meter connection failed:', err);
            return false;
        }
    }

    // ------------------------------------------------------------------
    // Data access
    // ------------------------------------------------------------------

    function getLatestSensorData() {
        return {
            heart_rate_bpm: latestHR,
            cadence_rpm: latestCadence,
            speed_kmh: latestSpeed,
            power_watts: latestPower,
        };
    }

    function isHRMConnected() {
        return hrmDevice && hrmDevice.gatt && hrmDevice.gatt.connected;
    }

    function isCSCConnected() {
        return cscDevice && cscDevice.gatt && cscDevice.gatt.connected;
    }

    function isPowerConnected() {
        return powerDevice && powerDevice.gatt && powerDevice.gatt.connected;
    }

    function updateSensorStatus(type, connected) {
        const el = document.getElementById(`sensor-${type}`);
        if (el) {
            el.classList.toggle('connected', connected);
        }
    }

    return {
        connectHRM,
        connectSpeedCadence,
        connectPowerMeter,
        getLatestSensorData,
        isHRMConnected,
        isCSCConnected,
        isPowerConnected,
    };
})();

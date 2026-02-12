/**
 * Sykla BLE (Web Bluetooth) glue layer.
 * Exposes functions to Rust/WASM via the global `syklaBle` namespace.
 *
 * FTMS Service UUID: 0x1826
 * Indoor Bike Data Characteristic: 0x2AD2 (notify)
 * Control Point Characteristic: 0x2AD9 (write)
 */

window.syklaBle = (() => {
    let device = null;
    let server = null;
    let controlPointChar = null;
    let connected = false;
    let latestData = null;

    const FTMS_SERVICE = 0x1826;
    const INDOOR_BIKE_DATA = 0x2AD2;
    const CONTROL_POINT = 0x2AD9;

    async function connectTrainer() {
        try {
            const btn = document.getElementById('connect-btn');
            btn.textContent = 'Connecting...';

            device = await navigator.bluetooth.requestDevice({
                filters: [{ services: [FTMS_SERVICE] }],
            });

            device.addEventListener('gattserverdisconnected', () => {
                connected = false;
                latestData = null;
                btn.textContent = 'Connect Trainer';
                btn.classList.remove('connected');
                console.log('[Sykla BLE] Disconnected');
            });

            server = await device.gatt.connect();
            const service = await server.getPrimaryService(FTMS_SERVICE);

            // Subscribe to Indoor Bike Data notifications
            const bikeDataChar = await service.getCharacteristic(INDOOR_BIKE_DATA);
            await bikeDataChar.startNotifications();
            bikeDataChar.addEventListener('characteristicvaluechanged', (event) => {
                const value = event.target.value;
                const data = parseIndoorBikeData(value);
                latestData = JSON.stringify(data);
            });

            // Get Control Point for writing simulation params
            controlPointChar = await service.getCharacteristic(CONTROL_POINT);

            connected = true;
            btn.textContent = 'Connected';
            btn.classList.add('connected');
            console.log('[Sykla BLE] Connected to', device.name);
        } catch (err) {
            console.error('[Sykla BLE] Connection failed:', err);
            const btn = document.getElementById('connect-btn');
            btn.textContent = 'Connect Trainer';
        }
    }

    function disconnectTrainer() {
        if (device && device.gatt.connected) {
            device.gatt.disconnect();
        }
        connected = false;
        latestData = null;
    }

    function isConnected() {
        return connected;
    }

    function getLatestData() {
        return latestData;
    }

    function writeSimParams(data) {
        if (!controlPointChar || !connected) return;
        controlPointChar.writeValue(new Uint8Array(data)).catch(err => {
            console.warn('[Sykla BLE] Write failed:', err);
        });
    }

    /**
     * Parse FTMS Indoor Bike Data characteristic value.
     * Returns { speed_kmh, cadence_rpm, power_watts, heart_rate_bpm }
     */
    function parseIndoorBikeData(dataView) {
        const flags = dataView.getUint16(0, true);
        let offset = 2;
        const result = { speed_kmh: 0, cadence_rpm: 0, power_watts: 0, heart_rate_bpm: null };

        // Instantaneous Speed (bit 0 NOT set = present)
        if ((flags & 0x01) === 0 && offset + 2 <= dataView.byteLength) {
            result.speed_kmh = dataView.getUint16(offset, true) * 0.01;
            offset += 2;
        }

        // Average Speed (bit 1)
        if (flags & 0x02) { offset += 2; }

        // Instantaneous Cadence (bit 2)
        if ((flags & 0x04) && offset + 2 <= dataView.byteLength) {
            result.cadence_rpm = dataView.getUint16(offset, true) * 0.5;
            offset += 2;
        }

        // Average Cadence (bit 3)
        if (flags & 0x08) { offset += 2; }

        // Total Distance (bit 4) — 3 bytes
        if (flags & 0x10) { offset += 3; }

        // Resistance Level (bit 5)
        if (flags & 0x20) { offset += 2; }

        // Instantaneous Power (bit 6)
        if ((flags & 0x40) && offset + 2 <= dataView.byteLength) {
            result.power_watts = dataView.getInt16(offset, true);
            offset += 2;
        }

        // Average Power (bit 7)
        if (flags & 0x80) { offset += 2; }

        // Expended Energy (bit 8) — 3 fields, 6 bytes
        if (flags & 0x100) { offset += 6; }

        // Heart Rate (bit 9)
        if ((flags & 0x200) && offset + 1 <= dataView.byteLength) {
            result.heart_rate_bpm = dataView.getUint8(offset);
        }

        return result;
    }

    return {
        connectTrainer,
        disconnectTrainer,
        isConnected,
        getLatestData,
        writeSimParams,
    };
})();

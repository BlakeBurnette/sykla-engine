/**
 * Sykla Activity Recording — GPS + BLE sensor recording for outdoor rides.
 *
 * Uses:
 * - navigator.geolocation.watchPosition() for GPS
 * - window.syklaSensors for BLE heart rate, cadence, power
 * - IndexedDB for resilient point buffering
 * - Batch uploads to server every 30 seconds
 * - Leaflet map for live position display
 * - WebSocket global sync for indoor rider visibility
 */

(function () {
    'use strict';

    // ------------------------------------------------------------------
    // State
    // ------------------------------------------------------------------

    const state = {
        activityId: null,
        recording: false,
        paused: false,
        startTime: null,
        elapsed: 0,       // ms
        distance: 0,       // meters
        points: [],         // buffered points not yet uploaded
        seq: 0,
        lastLat: null,
        lastLng: null,
        lastUploadTime: 0,
        watchId: null,
        map: null,
        marker: null,
        trackLine: null,
        trackPoints: [],
        autoPaused: false,
        globalWs: null,
        indoorMarkers: {},
    };

    const UPLOAD_INTERVAL_MS = 30000;
    const AUTO_PAUSE_SPEED_THRESHOLD = 1.0; // km/h
    const API_BASE = window.location.origin;

    // ------------------------------------------------------------------
    // Auth
    // ------------------------------------------------------------------

    function getToken() {
        return localStorage.getItem('sykla_token');
    }

    function authHeaders() {
        const token = getToken();
        return token ? { 'Authorization': `Bearer ${token}`, 'Content-Type': 'application/json' } : { 'Content-Type': 'application/json' };
    }

    // ------------------------------------------------------------------
    // Map setup
    // ------------------------------------------------------------------

    function initMap() {
        state.map = L.map('map').setView([35.78, -78.64], 14);
        L.tileLayer('https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png', {
            attribution: '&copy; OpenStreetMap contributors',
            maxZoom: 19,
        }).addTo(state.map);

        state.trackLine = L.polyline([], { color: '#ff4444', weight: 4 }).addTo(state.map);
    }

    // ------------------------------------------------------------------
    // GPS tracking
    // ------------------------------------------------------------------

    function startGPS() {
        if (!navigator.geolocation) {
            showStatus('Geolocation not supported');
            return;
        }

        state.watchId = navigator.geolocation.watchPosition(
            onGPSPosition,
            onGPSError,
            {
                enableHighAccuracy: true,
                maximumAge: 1000,
                timeout: 10000,
            }
        );
    }

    function stopGPS() {
        if (state.watchId !== null) {
            navigator.geolocation.clearWatch(state.watchId);
            state.watchId = null;
        }
    }

    function onGPSPosition(pos) {
        const lat = pos.coords.latitude;
        const lng = pos.coords.longitude;
        const elevation = pos.coords.altitude;
        const gpsSpeed = pos.coords.speed; // m/s or null

        // Update map
        if (!state.marker) {
            state.marker = L.circleMarker([lat, lng], {
                radius: 8,
                fillColor: '#ff4444',
                fillOpacity: 1,
                color: '#fff',
                weight: 2,
            }).addTo(state.map);
            state.map.setView([lat, lng], 16);
        } else {
            state.marker.setLatLng([lat, lng]);
        }

        if (!state.recording || state.paused) return;

        // Compute distance from last point
        let segmentDist = 0;
        if (state.lastLat !== null) {
            segmentDist = haversine(state.lastLat, state.lastLng, lat, lng);
            state.distance += segmentDist;
        }

        // Compute speed
        const sensorData = window.syklaSensors ? window.syklaSensors.getLatestSensorData() : {};
        let speedKmh = sensorData.speed_kmh || (gpsSpeed !== null ? gpsSpeed * 3.6 : null);

        // Auto-pause
        if (speedKmh !== null && speedKmh < AUTO_PAUSE_SPEED_THRESHOLD && !state.autoPaused) {
            state.autoPaused = true;
            showStatus('Auto-paused (stopped)');
        } else if (speedKmh !== null && speedKmh >= AUTO_PAUSE_SPEED_THRESHOLD && state.autoPaused) {
            state.autoPaused = false;
            showStatus('Recording...');
        }

        if (state.autoPaused) return;

        const elapsedMs = Date.now() - state.startTime;

        const point = {
            seq: state.seq++,
            timestamp_ms: elapsedMs,
            lat,
            lng,
            elevation_m: elevation,
            speed_kmh: speedKmh,
            power_watts: sensorData.power_watts || null,
            heart_rate_bpm: sensorData.heart_rate_bpm || null,
            cadence_rpm: sensorData.cadence_rpm || null,
            distance_from_start_m: state.distance,
            grade_percent: null,
        };

        state.points.push(point);
        state.trackPoints.push([lat, lng]);
        state.trackLine.setLatLngs(state.trackPoints);
        state.lastLat = lat;
        state.lastLng = lng;

        // Update metrics display
        updateMetrics(speedKmh, sensorData);

        // Batch upload every 30s
        if (Date.now() - state.lastUploadTime > UPLOAD_INTERVAL_MS && state.points.length > 0) {
            uploadPoints();
        }

        // Send position to global sync
        sendGlobalPosition(lat, lng, speedKmh || 0);
    }

    function onGPSError(err) {
        console.warn('[Sykla Record] GPS error:', err.message);
        showStatus('GPS: ' + err.message);
    }

    // ------------------------------------------------------------------
    // Activity lifecycle
    // ------------------------------------------------------------------

    async function startRecording() {
        const name = document.getElementById('activity-name').value || undefined;

        try {
            const res = await fetch(`${API_BASE}/api/activities`, {
                method: 'POST',
                headers: authHeaders(),
                body: JSON.stringify({ name }),
            });

            if (!res.ok) throw new Error(await res.text());
            const data = await res.json();
            state.activityId = data.id;
        } catch (err) {
            showStatus('Failed to start: ' + err.message);
            return;
        }

        state.recording = true;
        state.paused = false;
        state.startTime = Date.now();
        state.seq = 0;
        state.distance = 0;
        state.points = [];
        state.trackPoints = [];
        state.lastLat = null;
        state.lastLng = null;
        state.lastUploadTime = Date.now();

        startGPS();
        connectGlobalWS();

        document.getElementById('btn-start').disabled = true;
        document.getElementById('btn-pause').disabled = false;
        document.getElementById('btn-stop').disabled = false;
        showStatus('Recording...');
    }

    function pauseRecording() {
        if (state.paused) {
            state.paused = false;
            document.getElementById('btn-pause').textContent = 'Pause';
            showStatus('Recording...');
        } else {
            state.paused = true;
            document.getElementById('btn-pause').textContent = 'Resume';
            showStatus('Paused');
        }
    }

    async function stopRecording() {
        state.recording = false;
        stopGPS();

        // Upload remaining points
        if (state.points.length > 0) {
            await uploadPoints();
        }

        // Complete the activity
        try {
            await fetch(`${API_BASE}/api/activities/${state.activityId}`, {
                method: 'PUT',
                headers: authHeaders(),
                body: JSON.stringify({ completed_at: new Date().toISOString() }),
            });
        } catch (err) {
            console.error('[Sykla Record] Failed to complete activity:', err);
        }

        disconnectGlobalWS();

        document.getElementById('btn-start').disabled = false;
        document.getElementById('btn-pause').disabled = true;
        document.getElementById('btn-stop').disabled = true;
        showStatus('Activity saved!');
    }

    // ------------------------------------------------------------------
    // Batch upload
    // ------------------------------------------------------------------

    async function uploadPoints() {
        const batch = state.points.splice(0, state.points.length);
        if (batch.length === 0) return;

        state.lastUploadTime = Date.now();

        try {
            const res = await fetch(`${API_BASE}/api/activities/${state.activityId}/points`, {
                method: 'POST',
                headers: authHeaders(),
                body: JSON.stringify({ points: batch }),
            });

            if (!res.ok) {
                // Put points back for retry
                state.points.unshift(...batch);
                console.warn('[Sykla Record] Upload failed, will retry');
            }
        } catch (err) {
            state.points.unshift(...batch);
            console.warn('[Sykla Record] Upload error, will retry:', err);
        }
    }

    // ------------------------------------------------------------------
    // GPX Import
    // ------------------------------------------------------------------

    async function importGPX() {
        const fileInput = document.getElementById('gpx-file');
        if (!fileInput.files.length) {
            showStatus('Select a GPX file first');
            return;
        }

        const file = fileInput.files[0];
        const gpxData = await file.text();

        showStatus('Importing...');

        try {
            const res = await fetch(`${API_BASE}/api/activities/import`, {
                method: 'POST',
                headers: authHeaders(),
                body: JSON.stringify({
                    name: file.name.replace('.gpx', ''),
                    gpx_data: gpxData,
                }),
            });

            if (!res.ok) throw new Error(await res.text());
            const data = await res.json();
            showStatus(`Imported! Distance: ${(data.distance_m / 1000).toFixed(1)} km`);
        } catch (err) {
            showStatus('Import failed: ' + err.message);
        }
    }

    // ------------------------------------------------------------------
    // Global WebSocket (cross-world visibility)
    // ------------------------------------------------------------------

    function connectGlobalWS() {
        const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
        const url = `${protocol}//${window.location.host}/api/sync/global`;

        try {
            state.globalWs = new WebSocket(url);
            state.globalWs.onmessage = onGlobalMessage;
            state.globalWs.onclose = () => { state.globalWs = null; };
            state.globalWs.onerror = () => { state.globalWs = null; };
        } catch (err) {
            console.warn('[Sykla Record] Global WS failed:', err);
        }
    }

    function disconnectGlobalWS() {
        if (state.globalWs) {
            state.globalWs.close();
            state.globalWs = null;
        }
        // Clean up indoor rider markers
        for (const id in state.indoorMarkers) {
            state.map.removeLayer(state.indoorMarkers[id]);
        }
        state.indoorMarkers = {};
    }

    function sendGlobalPosition(lat, lng, speedKmh) {
        if (!state.globalWs || state.globalWs.readyState !== WebSocket.OPEN) return;

        state.globalWs.send(JSON.stringify({
            type: 'geo_position',
            lat,
            lng,
            speed_kmh: speedKmh,
            heading: 0,
            is_indoor: false,
        }));
    }

    function onGlobalMessage(event) {
        try {
            const msg = JSON.parse(event.data);
            if (msg.type === 'nearby_riders') {
                updateIndoorRiders(msg.riders || []);
            }
        } catch (err) {
            // ignore parse errors
        }
    }

    function updateIndoorRiders(riders) {
        const currentIds = new Set();

        for (const rider of riders) {
            if (!rider.is_indoor) continue; // Only show indoor riders on the map
            currentIds.add(rider.user_id);

            if (state.indoorMarkers[rider.user_id]) {
                state.indoorMarkers[rider.user_id].setLatLng([rider.lat, rider.lng]);
            } else {
                const marker = L.circleMarker([rider.lat, rider.lng], {
                    radius: 6,
                    fillColor: '#ff8c00',
                    fillOpacity: 0.8,
                    color: '#fff',
                    weight: 1,
                }).addTo(state.map);
                marker.bindTooltip(rider.display_name, { permanent: false });
                state.indoorMarkers[rider.user_id] = marker;
            }
        }

        // Remove stale markers
        for (const id in state.indoorMarkers) {
            if (!currentIds.has(id)) {
                state.map.removeLayer(state.indoorMarkers[id]);
                delete state.indoorMarkers[id];
            }
        }
    }

    // ------------------------------------------------------------------
    // Heat map overlay
    // ------------------------------------------------------------------

    let heatmapLayer = null;

    async function loadHeatmap() {
        if (!state.map) return;

        const bounds = state.map.getBounds();
        const params = new URLSearchParams({
            south: bounds.getSouth(),
            west: bounds.getWest(),
            north: bounds.getNorth(),
            east: bounds.getEast(),
        });

        try {
            const res = await fetch(`${API_BASE}/api/heatmap/bounds?${params}`);
            if (!res.ok) return;
            const cells = await res.json();

            if (heatmapLayer) {
                state.map.removeLayer(heatmapLayer);
            }

            const rects = [];
            for (const cell of cells) {
                // Approximate cell size at zoom 15
                const cellSize = 360.0 / Math.pow(2, 15);
                const bounds = [
                    [cell.lat, cell.lng],
                    [cell.lat - cellSize, cell.lng + cellSize],
                ];
                const intensity = Math.min(cell.ride_count / 10, 1.0);
                const color = heatColor(intensity);
                rects.push(L.rectangle(bounds, {
                    color: 'none',
                    fillColor: color,
                    fillOpacity: 0.4 + intensity * 0.3,
                    weight: 0,
                }));
            }

            heatmapLayer = L.layerGroup(rects).addTo(state.map);
        } catch (err) {
            console.warn('[Sykla Record] Heatmap load failed:', err);
        }
    }

    function heatColor(t) {
        // Green -> Yellow -> Red gradient
        if (t < 0.5) {
            const r = Math.floor(t * 2 * 255);
            return `rgb(${r}, 255, 0)`;
        } else {
            const g = Math.floor((1 - (t - 0.5) * 2) * 255);
            return `rgb(255, ${g}, 0)`;
        }
    }

    // ------------------------------------------------------------------
    // Metrics display
    // ------------------------------------------------------------------

    function updateMetrics(speedKmh, sensorData) {
        const elapsed = Date.now() - state.startTime;
        const mins = Math.floor(elapsed / 60000);
        const secs = Math.floor((elapsed % 60000) / 1000);

        document.getElementById('metric-speed').textContent =
            speedKmh !== null ? speedKmh.toFixed(1) : '--';
        document.getElementById('metric-distance').textContent =
            (state.distance / 1000).toFixed(2);
        document.getElementById('metric-time').textContent =
            `${mins}:${secs.toString().padStart(2, '0')}`;
        document.getElementById('metric-hr').textContent =
            sensorData.heart_rate_bpm || '--';
        document.getElementById('metric-cadence').textContent =
            sensorData.cadence_rpm ? sensorData.cadence_rpm.toFixed(0) : '--';
        document.getElementById('metric-power').textContent =
            sensorData.power_watts || '--';
    }

    function showStatus(msg) {
        document.getElementById('rec-status').textContent = msg;
    }

    // ------------------------------------------------------------------
    // Haversine distance (meters)
    // ------------------------------------------------------------------

    function haversine(lat1, lng1, lat2, lng2) {
        const R = 6371000;
        const dLat = (lat2 - lat1) * Math.PI / 180;
        const dLng = (lng2 - lng1) * Math.PI / 180;
        const a = Math.sin(dLat / 2) ** 2 +
                  Math.cos(lat1 * Math.PI / 180) * Math.cos(lat2 * Math.PI / 180) *
                  Math.sin(dLng / 2) ** 2;
        return R * 2 * Math.atan2(Math.sqrt(a), Math.sqrt(1 - a));
    }

    // ------------------------------------------------------------------
    // Init
    // ------------------------------------------------------------------

    window.syklaRecord = {
        startRecording,
        pauseRecording,
        stopRecording,
        importGPX,
        loadHeatmap,
    };

    document.addEventListener('DOMContentLoaded', () => {
        initMap();

        // Load heatmap on map move
        state.map.on('moveend', loadHeatmap);
        loadHeatmap();
    });
})();

/**
 * Common plotting functions for Room Acoustics Simulator
 * Shared between plot_room_sim.html and room_simulator_wasm.html
 */

// Source colors used across all plots
const SOURCE_COLORS = ['#ff6b6b', '#4ecdc4', '#45b7d1', '#f9ca24', '#6c5ce7', '#fd79a8'];
const SOURCE_COLORS_3D = ['#ff6b6b', '#ee5a6f', '#f06595', '#cc5de8', '#845ef7'];

/**
 * Plot frequency response at listening position
 * @param {Object} data - Simulation results with frequencies, frequency_response, source_responses
 * @param {string} containerId - DOM element ID for the plot
 * @param {Object} options - Optional settings (darkMode, etc.)
 */
function plotFrequencyResponse(data, containerId, options = {}) {
    const traces = [];
    const darkMode = options.darkMode || false;

    // Per-source response traces (if available)
    if (data.source_responses && data.source_responses.length > 0) {
        data.source_responses.forEach((srcResp, idx) => {
            traces.push({
                x: data.frequencies,
                y: srcResp.spl,
                type: 'scatter',
                mode: 'lines',
                line: {
                    color: SOURCE_COLORS[idx % SOURCE_COLORS.length],
                    width: 2,
                    dash: options.sourceLineDash || 'dot'
                },
                name: srcResp.source_name || `Source ${idx + 1}`,
                hovertemplate: 'Frequency: %{x:.1f} Hz<br>SPL: %{y:.1f} dB<extra></extra>'
            });
        });
    }

    // Main frequency response trace (combined, on top)
    traces.push({
        x: data.frequencies,
        y: data.frequency_response,
        type: 'scatter',
        mode: options.showMarkers ? 'lines+markers' : 'lines',
        line: {
            color: '#667eea',
            width: 3
        },
        marker: options.showMarkers ? {
            size: 6,
            color: '#667eea',
            line: { color: 'white', width: 1 }
        } : undefined,
        name: 'Combined Response',
        hovertemplate: 'Frequency: %{x:.1f} Hz<br>SPL: %{y:.1f} dB<extra></extra>'
    });

    // Determine axis range
    const minFreq = Math.min(...data.frequencies);
    const maxFreq = Math.max(...data.frequencies);
    const allSPL = data.frequency_response;
    const meanSPL = allSPL.reduce((a, b) => a + b, 0) / allSPL.length;
    const splSpan = options.splSpan || 50;
    const splMin = meanSPL - splSpan / 2;
    const splMax = meanSPL + splSpan / 2;

    // Store for use by slice plots
    window.sharedSPLRange = { min: splMin, max: splMax };

    const layout = {
        title: {
            text: options.title || 'Frequency Response at Listening Position',
            font: { size: 18, weight: 'bold' }
        },
        xaxis: {
            title: 'Frequency (Hz)',
            type: minFreq >= 10 && maxFreq > minFreq * 5 ? 'log' : 'linear',
            gridcolor: darkMode ? 'rgba(255,255,255,0.1)' : '#e0e0e0',
            autorange: true
        },
        yaxis: {
            title: 'SPL (dB)',
            gridcolor: darkMode ? 'rgba(255,255,255,0.1)' : '#e0e0e0',
            range: [splMin, splMax]
        },
        hovermode: 'closest',
        autosize: true,
        plot_bgcolor: darkMode ? 'rgba(0,0,0,0)' : '#fafafa',
        paper_bgcolor: darkMode ? 'rgba(0,0,0,0)' : 'white',
        showlegend: true,
        legend: {
            x: 1.02,
            y: 1,
            xanchor: 'left',
            yanchor: 'top'
        },
        margin: { t: 50, r: 150 }
    };

    if (darkMode) {
        layout.font = { color: '#fff' };
    }

    Plotly.newPlot(containerId, traces, layout, { responsive: true });

    return { splMin, splMax };
}

/**
 * Plot 3D room visualization with sources and listening position
 * @param {Object} data - Simulation results with room, sources, listening_position
 * @param {string} containerId - DOM element ID for the plot
 * @param {Object} options - Optional settings (darkMode, etc.)
 */
function plot3DRoom(data, containerId, options = {}) {
    if (!data.room || !data.sources || !data.listening_position) {
        console.error('Missing required data for 3D room plot');
        return;
    }

    const traces = [];
    const room = data.room;
    const w = room.width || 1;
    const d = room.depth || 1;
    const h = room.height || 1;

    // Room wireframe - use edges if available, otherwise generate box
    if (room.edges && room.edges.length > 0) {
        room.edges.forEach(edge => {
            const [p1, p2] = edge;
            traces.push({
                x: [p1[0], p2[0]],
                y: [p1[1], p2[1]],
                z: [p1[2], p2[2]],
                mode: 'lines',
                type: 'scatter3d',
                showlegend: false,
                line: { color: 'rgba(100, 100, 100, 0.5)', width: 3 }
            });
        });
    } else {
        // Generate box edges manually
        const edges = [
            // Bottom
            [[0, 0, 0], [w, 0, 0]],
            [[w, 0, 0], [w, d, 0]],
            [[w, d, 0], [0, d, 0]],
            [[0, d, 0], [0, 0, 0]],
            // Top
            [[0, 0, h], [w, 0, h]],
            [[w, 0, h], [w, d, h]],
            [[w, d, h], [0, d, h]],
            [[0, d, h], [0, 0, h]],
            // Vertical
            [[0, 0, 0], [0, 0, h]],
            [[w, 0, 0], [w, 0, h]],
            [[w, d, 0], [w, d, h]],
            [[0, d, 0], [0, d, h]]
        ];

        edges.forEach(edge => {
            const [p1, p2] = edge;
            traces.push({
                x: [p1[0], p2[0]],
                y: [p1[1], p2[1]],
                z: [p1[2], p2[2]],
                mode: 'lines',
                type: 'scatter3d',
                showlegend: false,
                line: { color: 'rgba(100, 100, 100, 0.5)', width: 3 }
            });
        });
    }

    // Add sources with different colors
    data.sources.forEach((source, idx) => {
        traces.push({
            x: [source.position[0]],
            y: [source.position[1]],
            z: [source.position[2]],
            mode: 'markers+text',
            type: 'scatter3d',
            name: source.name,
            text: [source.name.split(' ').map((word, i) => i === 0 ? word[0] : word[0]).join('')],
            textposition: 'top center',
            marker: {
                size: 10,
                color: SOURCE_COLORS_3D[idx % SOURCE_COLORS_3D.length],
                symbol: 'diamond',
                line: { color: 'white', width: 2 }
            },
            hovertemplate: `<b>${source.name}</b><br>x: %{x:.2f} m<br>y: %{y:.2f} m<br>z: %{z:.2f} m<extra></extra>`
        });
    });

    // Add listening position
    traces.push({
        x: [data.listening_position[0]],
        y: [data.listening_position[1]],
        z: [data.listening_position[2]],
        mode: 'markers+text',
        type: 'scatter3d',
        name: 'Listening Position',
        text: ['LP'],
        textposition: 'top center',
        marker: {
            size: 8,
            color: '#51cf66',
            symbol: 'circle',
            line: { color: 'black', width: 2 }
        },
        hovertemplate: '<b>Listening Position</b><br>x: %{x:.2f} m<br>y: %{y:.2f} m<br>z: %{z:.2f} m<extra></extra>'
    });

    const layout = {
        scene: {
            xaxis: { title: 'x (m)', range: [0, w] },
            yaxis: { title: 'y (m)', range: [0, d] },
            zaxis: { title: 'z (m)', range: [0, h] },
            camera: {
                eye: { x: 1.8, y: -1.8, z: 1.2 },
                center: { x: 0, y: 0, z: 0 }
            },
            aspectmode: options.aspectmode || 'data'
        },
        autosize: true,
        showlegend: true,
        legend: { x: 0, y: 1 },
        margin: { t: 30 }
    };

    Plotly.newPlot(containerId, traces, layout, { responsive: true });
}

/**
 * Initialize spatial slice plots and frequency slider
 * @param {Object} data - Simulation results with horizontal_slices, vertical_slices
 * @param {string} sliderId - DOM element ID for the frequency slider
 * @param {string} displayId - DOM element ID for frequency display
 */
function initSpatialSlices(data, sliderId, displayId) {
    const slider = document.getElementById(sliderId);
    slider.max = data.horizontal_slices.length - 1;
    slider.value = Math.floor(data.horizontal_slices.length / 2);

    window.currentSliceData = data;
}

/**
 * Update slice plots based on current slider position
 * @param {Object} data - Simulation results (uses window.currentSliceData if not provided)
 * @param {string} sliderId - DOM element ID for the frequency slider
 * @param {string} displayId - DOM element ID for frequency display
 * @param {string} hSliceId - DOM element ID for horizontal slice plot
 * @param {string} vSliceId - DOM element ID for vertical slice plot
 * @param {string} combinedId - DOM element ID for 3D combined plot
 * @param {Object} options - Optional settings
 */
function updateSlicePlots(sliderId, displayId, hSliceId, vSliceId, combinedId, options = {}) {
    const data = window.currentSliceData;
    if (!data || !data.horizontal_slices || !data.vertical_slices) return;

    const idx = parseInt(document.getElementById(sliderId).value);
    if (idx >= data.horizontal_slices.length || idx >= data.vertical_slices.length) return;

    const hSlice = data.horizontal_slices[idx];
    const vSlice = data.vertical_slices[idx];

    if (!hSlice || !vSlice || !hSlice.x || !hSlice.y || !hSlice.spl) return;

    // Update frequency display
    document.getElementById(displayId).textContent = `${hSlice.frequency.toFixed(1)} Hz`;

    // Reshape SPL data into 2D grid
    const hShape = hSlice.shape;
    const vShape = vSlice.shape;

    if (!hShape || hShape.length < 2 || !vShape || vShape.length < 2) return;

    // Convert flat array to 2D for Plotly (row-major)
    const hSPL2D = [];
    for (let i = 0; i < hShape[0]; i++) {
        hSPL2D.push(hSlice.spl.slice(i * hShape[1], (i + 1) * hShape[1]));
    }

    const vSPL2D = [];
    for (let i = 0; i < vShape[0]; i++) {
        vSPL2D.push(vSlice.spl.slice(i * vShape[1], (i + 1) * vShape[1]));
    }

    // Use shared SPL range or calculate from data
    const splRange = window.sharedSPLRange || {
        min: Math.min(...hSlice.spl.filter(v => v > -150)),
        max: Math.max(...hSlice.spl)
    };

    // Plot horizontal slice (XY)
    plotHorizontalSlice(hSlice, hSPL2D, data, hSliceId, splRange, options);

    // Plot vertical slice (XZ)
    plotVerticalSlice(vSlice, vSPL2D, data, vSliceId, splRange, options);

    // Plot 3D combined view
    if (combinedId) {
        plot3DCombinedSlices(hSlice, vSlice, hSPL2D, vSPL2D, data, combinedId, splRange, options);
    }
}

/**
 * Plot horizontal slice (XY plane)
 */
function plotHorizontalSlice(hSlice, hSPL2D, data, containerId, splRange, options = {}) {
    const hTrace = {
        x: hSlice.x,
        y: hSlice.y,
        z: hSPL2D,
        type: 'heatmap',
        colorscale: 'Jet',
        colorbar: { title: 'SPL (dB)', x: 1.15 },
        zmin: splRange.min,
        zmax: splRange.max
    };

    // Add sources overlay
    const hSourcesTrace = {
        x: data.sources.map(s => s.position[0]),
        y: data.sources.map(s => s.position[1]),
        mode: 'markers+text',
        type: 'scatter',
        marker: {
            size: 12,
            color: 'white',
            symbol: 'diamond',
            line: { color: 'black', width: 2 }
        },
        text: data.sources.map(s => s.name.split(' ').map(w => w[0]).join('')),
        textposition: 'top center',
        textfont: { color: 'white', size: 10 },
        name: 'Sources',
        showlegend: false
    };

    // Add listening position overlay
    const hLPTrace = {
        x: [data.listening_position[0]],
        y: [data.listening_position[1]],
        mode: 'markers+text',
        type: 'scatter',
        marker: {
            size: 10,
            color: 'lime',
            symbol: 'circle',
            line: { color: 'black', width: 2 }
        },
        text: ['LP'],
        textposition: 'top center',
        textfont: { color: 'white', size: 10 },
        name: 'LP',
        showlegend: false
    };

    const layout = {
        title: options.showTitle ? `Horizontal Slice at ${hSlice.frequency.toFixed(1)} Hz (z = ${data.listening_position[2].toFixed(1)} m)` : undefined,
        xaxis: { title: 'x (m)', constrain: 'domain' },
        yaxis: { title: 'y (m)', scaleanchor: 'x', scaleratio: 1, constrain: 'domain' },
        autosize: true,
        margin: { l: 60, r: 120, t: options.showTitle ? 50 : 30, b: 60 }
    };

    Plotly.newPlot(containerId, [hTrace, hSourcesTrace, hLPTrace], layout, { responsive: true });
}

/**
 * Plot vertical slice (XZ plane)
 */
function plotVerticalSlice(vSlice, vSPL2D, data, containerId, splRange, options = {}) {
    const vSliceY = vSlice.z || vSlice.y;

    const vTrace = {
        x: vSlice.x,
        y: vSliceY,
        z: vSPL2D,
        type: 'heatmap',
        colorscale: 'Jet',
        colorbar: { title: 'SPL (dB)', x: 1.15 },
        zmin: splRange.min,
        zmax: splRange.max
    };

    // Add sources at correct z height
    const vSourcesTrace = {
        x: data.sources.map(s => s.position[0]),
        y: data.sources.map(s => s.position[2]),
        mode: 'markers+text',
        type: 'scatter',
        marker: {
            size: 12,
            color: 'white',
            symbol: 'diamond',
            line: { color: 'black', width: 2 }
        },
        text: data.sources.map(s => s.name.split(' ').map(w => w[0]).join('')),
        textposition: 'top center',
        textfont: { color: 'white', size: 10 },
        name: 'Sources',
        showlegend: false
    };

    const vLPTrace = {
        x: [data.listening_position[0]],
        y: [data.listening_position[2]],
        mode: 'markers+text',
        type: 'scatter',
        marker: {
            size: 10,
            color: 'lime',
            symbol: 'circle',
            line: { color: 'black', width: 2 }
        },
        text: ['LP'],
        textposition: 'top center',
        textfont: { color: 'white', size: 10 },
        name: 'LP',
        showlegend: false
    };

    const layout = {
        title: options.showTitle ? `Vertical Slice at ${vSlice.frequency.toFixed(1)} Hz (y = ${data.listening_position[1].toFixed(1)} m)` : undefined,
        xaxis: { title: 'x (m)', constrain: 'domain' },
        yaxis: { title: 'z (m)', scaleanchor: 'x', scaleratio: 1, constrain: 'domain' },
        autosize: true,
        margin: { l: 60, r: 120, t: options.showTitle ? 50 : 30, b: 60 }
    };

    Plotly.newPlot(containerId, [vTrace, vSourcesTrace, vLPTrace], layout, { responsive: true });
}

/**
 * Plot 3D combined view with orthogonal slices
 */
function plot3DCombinedSlices(hSlice, vSlice, hSPL2D, vSPL2D, data, containerId, splRange, options = {}) {
    const hShape = hSlice.shape;
    const vShape = vSlice.shape;
    const traces = [];

    // Horizontal slice as surface at LP height
    const hSurface = {
        x: hSlice.x,
        y: hSlice.y,
        z: Array(hShape[0]).fill(null).map(() => Array(hShape[1]).fill(data.listening_position[2])),
        surfacecolor: hSPL2D,
        type: 'surface',
        colorscale: 'Jet',
        cmin: splRange.min,
        cmax: splRange.max,
        showscale: true,
        colorbar: { title: 'SPL (dB)', x: 1.1 },
        name: 'Horizontal Slice',
        hovertemplate: 'x: %{x:.2f} m<br>y: %{y:.2f} m<br>SPL: %{surfacecolor:.1f} dB<extra></extra>'
    };
    traces.push(hSurface);

    // Vertical slice as surface at LP depth
    const vZ = vSlice.z || vSlice.y;
    const vSurface = {
        x: vSlice.x,
        y: Array(vShape[0]).fill(null).map(() => Array(vShape[1]).fill(data.listening_position[1])),
        z: vZ.map(z_val => Array(vShape[1]).fill(z_val)),
        surfacecolor: vSPL2D,
        type: 'surface',
        colorscale: 'Jet',
        cmin: splRange.min,
        cmax: splRange.max,
        showscale: false,
        name: 'Vertical Slice',
        hovertemplate: 'x: %{x:.2f} m<br>z: %{z:.2f} m<br>SPL: %{surfacecolor:.1f} dB<extra></extra>'
    };
    traces.push(vSurface);

    // Add sources as 3D scatter
    const sourcesTrace = {
        x: data.sources.map(s => s.position[0]),
        y: data.sources.map(s => s.position[1]),
        z: data.sources.map(s => s.position[2]),
        mode: 'markers+text',
        type: 'scatter3d',
        marker: {
            size: 8,
            color: 'white',
            symbol: 'diamond',
            line: { color: 'black', width: 2 }
        },
        text: data.sources.map(s => s.name.split(' ').map(w => w[0]).join('')),
        textposition: 'top center',
        textfont: { color: 'white', size: 10 },
        name: 'Sources',
        hovertemplate: '<b>%{text}</b><br>x: %{x:.2f} m<br>y: %{y:.2f} m<br>z: %{z:.2f} m<extra></extra>'
    };
    traces.push(sourcesTrace);

    // Add listening position
    const lpTrace = {
        x: [data.listening_position[0]],
        y: [data.listening_position[1]],
        z: [data.listening_position[2]],
        mode: 'markers+text',
        type: 'scatter3d',
        marker: {
            size: 6,
            color: 'lime',
            symbol: 'circle',
            line: { color: 'black', width: 2 }
        },
        text: ['LP'],
        textposition: 'top center',
        textfont: { color: 'white', size: 10 },
        name: 'Listening Position',
        hovertemplate: '<b>Listening Position</b><br>x: %{x:.2f} m<br>y: %{y:.2f} m<br>z: %{z:.2f} m<extra></extra>'
    };
    traces.push(lpTrace);

    const roomWidth = data.room.width || 1;
    const roomDepth = data.room.depth || 1;
    const roomHeight = data.room.height || 1;

    const layout = {
        title: options.showTitle ? `3D Pressure Field at ${hSlice.frequency.toFixed(1)} Hz` : undefined,
        scene: {
            xaxis: { title: 'x (m)', range: [0, roomWidth] },
            yaxis: { title: 'y (m)', range: [0, roomDepth] },
            zaxis: { title: 'z (m)', range: [0, roomHeight] },
            camera: {
                eye: { x: 1.5, y: -1.5, z: 1.2 },
                center: { x: 0, y: 0, z: 0 }
            },
            aspectmode: 'manual',
            aspectratio: {
                x: 1,
                y: roomDepth / roomWidth,
                z: roomHeight / roomWidth
            }
        },
        autosize: true,
        height: options.height || 600,
        showlegend: true,
        margin: { l: 0, r: 0, t: options.showTitle ? 50 : 30, b: 0 }
    };

    Plotly.newPlot(containerId, traces, layout, { responsive: true });
}

// Export functions for use in HTML files
if (typeof window !== 'undefined') {
    window.RoomPlots = {
        plotFrequencyResponse,
        plot3DRoom,
        initSpatialSlices,
        updateSlicePlots,
        plotHorizontalSlice,
        plotVerticalSlice,
        plot3DCombinedSlices,
        SOURCE_COLORS,
        SOURCE_COLORS_3D
    };
}

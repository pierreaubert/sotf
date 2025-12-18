//! Export functions for convex hulls

use crate::types::ConvexHull3D;
use std::fs::File;
use std::io::Write;
use std::path::Path;

/// Export a convex hull to OBJ format
///
/// The OBJ format is a simple text format for 3D models.
/// It includes vertices (v), normals (vn), and faces (f).
pub fn export_obj<P: AsRef<Path>>(hull: &ConvexHull3D, path: P) -> std::io::Result<()> {
    let mut file = File::create(path)?;

    // Write header
    writeln!(file, "# Convex Hull OBJ Export")?;
    writeln!(file, "# Vertices: {}", hull.num_vertices())?;
    writeln!(file, "# Faces: {}", hull.num_faces())?;
    writeln!(file)?;

    // Write vertices
    for vertex in hull.vertices() {
        writeln!(file, "v {} {} {}", vertex.x, vertex.y, vertex.z)?;
    }

    writeln!(file)?;

    // Write normals
    for face in hull.faces() {
        let normal = face.normal(hull.vertices());
        writeln!(file, "vn {} {} {}", normal.x, normal.y, normal.z)?;
    }

    writeln!(file)?;

    // Write faces (OBJ uses 1-based indexing)
    for (i, face) in hull.faces().iter().enumerate() {
        writeln!(
            file,
            "f {}//{} {}//{} {}//{}",
            face.v0 + 1,
            i + 1,
            face.v1 + 1,
            i + 1,
            face.v2 + 1,
            i + 1
        )?;
    }

    Ok(())
}

/// Export a convex hull to HTML with Three.js visualization
pub fn export_html<P: AsRef<Path>>(
    hull: &ConvexHull3D,
    path: P,
    title: &str,
) -> std::io::Result<()> {
    let mut file = File::create(path)?;

    // Convert hull to JSON
    let vertices_json = hull
        .vertices()
        .iter()
        .map(|v| format!("[{}, {}, {}]", v.x, v.y, v.z))
        .collect::<Vec<_>>()
        .join(",\n        ");

    let faces_json = hull
        .faces()
        .iter()
        .map(|f| format!("[{}, {}, {}]", f.v0, f.v1, f.v2))
        .collect::<Vec<_>>()
        .join(",\n        ");

    let html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{title}</title>
    <style>
        body {{
            margin: 0;
            overflow: hidden;
            font-family: Arial, sans-serif;
        }}
        #info {{
            position: absolute;
            top: 10px;
            left: 10px;
            background: rgba(0, 0, 0, 0.7);
            color: white;
            padding: 15px;
            border-radius: 5px;
            font-size: 14px;
            z-index: 100;
        }}
        #info h2 {{
            margin: 0 0 10px 0;
            font-size: 18px;
        }}
        #info p {{
            margin: 5px 0;
        }}
        #container {{
            width: 100vw;
            height: 100vh;
            display: flex;
            flex-direction: row;
        }}
        .viewport {{
            width: 50%;
            height: 100%;
            position: relative;
        }}
        .label {{
            position: absolute;
            top: 10px;
            left: 50%;
            transform: translateX(-50%);
            background: rgba(0, 0, 0, 0.7);
            color: white;
            padding: 8px 15px;
            border-radius: 5px;
            font-size: 14px;
            z-index: 10;
            font-weight: bold;
        }}
    </style>
</head>
<body>
    <div id="info">
        <h2>{title}</h2>
        <p>Vertices: {num_vertices}</p>
        <p>Faces: {num_faces}</p>
        <p>Volume: {volume:.6}</p>
        <p>Surface Area: {surface_area:.6}</p>
        <p><small>Synchronized views - rotate one to rotate both</small></p>
    </div>
    <div id="container">
        <div class="viewport" id="viewport-left">
            <div class="label">Original Points</div>
        </div>
        <div class="viewport" id="viewport-right">
            <div class="label">Convex Hull</div>
        </div>
    </div>

    <script type="importmap">
    {{
        "imports": {{
            "three": "https://cdn.jsdelivr.net/npm/three@0.170.0/build/three.module.js",
            "three/addons/": "https://cdn.jsdelivr.net/npm/three@0.170.0/examples/jsm/"
        }}
    }}
    </script>

    <script type="module">
        import * as THREE from 'three';
        import {{ OrbitControls }} from 'three/addons/controls/OrbitControls.js';

        // Hull data
        const vertices = [
        {vertices_json}
        ];

        const faces = [
        {faces_json}
        ];

        // Create geometry
        const geometry = new THREE.BufferGeometry();
        const positions = new Float32Array(vertices.flat());
        geometry.setAttribute('position', new THREE.BufferAttribute(positions, 3));
        const indices = new Uint32Array(faces.flat());
        geometry.setIndex(new THREE.BufferAttribute(indices, 1));
        geometry.computeVertexNormals();

        // Calculate bounding box for camera positioning
        geometry.computeBoundingBox();
        const box = geometry.boundingBox;
        const center = new THREE.Vector3();
        box.getCenter(center);
        const size = new THREE.Vector3();
        box.getSize(size);
        const maxDim = Math.max(size.x, size.y, size.z);

        // ===== LEFT SCENE: Original Points =====
        const sceneLeft = new THREE.Scene();
        sceneLeft.background = new THREE.Color(0x1a1a1a);

        const cameraLeft = new THREE.PerspectiveCamera(
            75,
            window.innerWidth / 2 / window.innerHeight,
            0.1,
            1000
        );

        const rendererLeft = new THREE.WebGLRenderer({{ antialias: true }});
        rendererLeft.setSize(window.innerWidth / 2, window.innerHeight);
        document.getElementById('viewport-left').appendChild(rendererLeft.domElement);

        // Add convex hull faces with semi-transparent material
        const materialLeft = new THREE.MeshPhongMaterial({{
            color: 0x3498db,
            side: THREE.DoubleSide,
            flatShading: false,
            transparent: true,
            opacity: 0.3
        }});
        const meshLeft = new THREE.Mesh(geometry, materialLeft);
        sceneLeft.add(meshLeft);

        // Add points (original vertices) on top
        const pointsMaterialLeft = new THREE.PointsMaterial({{
            color: 0xff6b6b,
            size: 0.08,
            sizeAttenuation: true
        }});
        const pointsLeft = new THREE.Points(geometry, pointsMaterialLeft);
        sceneLeft.add(pointsLeft);

        // Add lights
        const ambientLightLeft = new THREE.AmbientLight(0x404040, 2);
        sceneLeft.add(ambientLightLeft);

        const directionalLight1Left = new THREE.DirectionalLight(0xffffff, 1);
        directionalLight1Left.position.set(5, 5, 5);
        sceneLeft.add(directionalLight1Left);

        const directionalLight2Left = new THREE.DirectionalLight(0xffffff, 0.5);
        directionalLight2Left.position.set(-5, -5, -5);
        sceneLeft.add(directionalLight2Left);

        // Add axes helper
        const axesHelperLeft = new THREE.AxesHelper(maxDim * 0.5);
        sceneLeft.add(axesHelperLeft);

        // Add grid
        const gridHelperLeft = new THREE.GridHelper(maxDim * 2, 10, 0x444444, 0x222222);
        sceneLeft.add(gridHelperLeft);

        // Position camera
        const fov = cameraLeft.fov * (Math.PI / 180);
        let cameraZ = Math.abs(maxDim / 2 / Math.tan(fov / 2));
        cameraZ *= 2.5;

        cameraLeft.position.set(center.x + cameraZ * 0.5, center.y + cameraZ * 0.5, center.z + cameraZ);
        cameraLeft.lookAt(center);

        const controlsLeft = new OrbitControls(cameraLeft, rendererLeft.domElement);
        controlsLeft.target.copy(center);
        controlsLeft.enableDamping = true;
        controlsLeft.dampingFactor = 0.05;

        // ===== RIGHT SCENE: Convex Hull =====
        const sceneRight = new THREE.Scene();
        sceneRight.background = new THREE.Color(0x1a1a1a);

        const cameraRight = new THREE.PerspectiveCamera(
            75,
            window.innerWidth / 2 / window.innerHeight,
            0.1,
            1000
        );

        const rendererRight = new THREE.WebGLRenderer({{ antialias: true }});
        rendererRight.setSize(window.innerWidth / 2, window.innerHeight);
        document.getElementById('viewport-right').appendChild(rendererRight.domElement);

        // Create mesh with material
        const material = new THREE.MeshPhongMaterial({{
            color: 0x3498db,
            side: THREE.DoubleSide,
            flatShading: false,
            transparent: true,
            opacity: 0.8
        }});

        const mesh = new THREE.Mesh(geometry, material);
        sceneRight.add(mesh);

        // Add wireframe
        const wireframe = new THREE.WireframeGeometry(geometry);
        const line = new THREE.LineSegments(wireframe);
        line.material.color = new THREE.Color(0xffffff);
        line.material.opacity = 0.3;
        line.material.transparent = true;
        sceneRight.add(line);

        // Add points to hull view too
        const pointsMaterialRight = new THREE.PointsMaterial({{
            color: 0xff6b6b,
            size: 0.05,
            sizeAttenuation: true
        }});
        const pointsRight = new THREE.Points(geometry, pointsMaterialRight);
        sceneRight.add(pointsRight);

        // Add lights
        const ambientLightRight = new THREE.AmbientLight(0x404040, 2);
        sceneRight.add(ambientLightRight);

        const directionalLight1Right = new THREE.DirectionalLight(0xffffff, 1);
        directionalLight1Right.position.set(5, 5, 5);
        sceneRight.add(directionalLight1Right);

        const directionalLight2Right = new THREE.DirectionalLight(0xffffff, 0.5);
        directionalLight2Right.position.set(-5, -5, -5);
        sceneRight.add(directionalLight2Right);

        // Add axes helper
        const axesHelperRight = new THREE.AxesHelper(maxDim * 0.5);
        sceneRight.add(axesHelperRight);

        // Add grid
        const gridHelperRight = new THREE.GridHelper(maxDim * 2, 10, 0x444444, 0x222222);
        sceneRight.add(gridHelperRight);

        // Position camera (same as left)
        cameraRight.position.copy(cameraLeft.position);
        cameraRight.lookAt(center);

        const controlsRight = new OrbitControls(cameraRight, rendererRight.domElement);
        controlsRight.target.copy(center);
        controlsRight.enableDamping = true;
        controlsRight.dampingFactor = 0.05;

        // ===== SYNCHRONIZE CONTROLS =====
        function syncControls(sourceControls, sourceCamera, targetControls, targetCamera) {{
            targetCamera.position.copy(sourceCamera.position);
            targetCamera.rotation.copy(sourceCamera.rotation);
            targetCamera.quaternion.copy(sourceCamera.quaternion);
            targetControls.target.copy(sourceControls.target);
            targetCamera.updateProjectionMatrix();
        }}

        controlsLeft.addEventListener('change', () => {{
            syncControls(controlsLeft, cameraLeft, controlsRight, cameraRight);
        }});

        controlsRight.addEventListener('change', () => {{
            syncControls(controlsRight, cameraRight, controlsLeft, cameraLeft);
        }});

        // Handle window resize
        window.addEventListener('resize', () => {{
            const width = window.innerWidth / 2;
            const height = window.innerHeight;

            cameraLeft.aspect = width / height;
            cameraLeft.updateProjectionMatrix();
            rendererLeft.setSize(width, height);

            cameraRight.aspect = width / height;
            cameraRight.updateProjectionMatrix();
            rendererRight.setSize(width, height);
        }});

        // Animation loop
        function animate() {{
            requestAnimationFrame(animate);

            controlsLeft.update();
            controlsRight.update();

            rendererLeft.render(sceneLeft, cameraLeft);
            rendererRight.render(sceneRight, cameraRight);
        }}

        animate();
    </script>
</body>
</html>"#,
        title = title,
        num_vertices = hull.num_vertices(),
        num_faces = hull.num_faces(),
        volume = hull.volume(),
        surface_area = hull.surface_area(),
        vertices_json = vertices_json,
        faces_json = faces_json
    );

    file.write_all(html.as_bytes())?;

    Ok(())
}

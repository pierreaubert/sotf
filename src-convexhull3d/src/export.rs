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
        <p><small>Click and drag to rotate, scroll to zoom</small></p>
    </div>
    <div id="container"></div>

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

        // Setup scene
        const scene = new THREE.Scene();
        scene.background = new THREE.Color(0x1a1a1a);

        const camera = new THREE.PerspectiveCamera(
            75,
            window.innerWidth / window.innerHeight,
            0.1,
            1000
        );

        const renderer = new THREE.WebGLRenderer({{ antialias: true }});
        renderer.setSize(window.innerWidth, window.innerHeight);
        document.getElementById('container').appendChild(renderer.domElement);

        // Create geometry
        const geometry = new THREE.BufferGeometry();

        // Convert vertices to Float32Array
        const positions = new Float32Array(vertices.flat());
        geometry.setAttribute('position', new THREE.BufferAttribute(positions, 3));

        // Convert faces to indices
        const indices = new Uint32Array(faces.flat());
        geometry.setIndex(new THREE.BufferAttribute(indices, 1));

        // Compute normals for lighting
        geometry.computeVertexNormals();

        // Create mesh with material
        const material = new THREE.MeshPhongMaterial({{
            color: 0x3498db,
            side: THREE.DoubleSide,
            flatShading: false,
            transparent: true,
            opacity: 0.8
        }});

        const mesh = new THREE.Mesh(geometry, material);
        scene.add(mesh);

        // Add wireframe
        const wireframe = new THREE.WireframeGeometry(geometry);
        const line = new THREE.LineSegments(wireframe);
        line.material.color = new THREE.Color(0xffffff);
        line.material.opacity = 0.3;
        line.material.transparent = true;
        scene.add(line);

        // Add point cloud
        const pointsMaterial = new THREE.PointsMaterial({{
            color: 0xff6b6b,
            size: 0.05,
            sizeAttenuation: true
        }});
        const points = new THREE.Points(geometry, pointsMaterial);
        scene.add(points);

        // Add lights
        const ambientLight = new THREE.AmbientLight(0x404040, 2);
        scene.add(ambientLight);

        const directionalLight1 = new THREE.DirectionalLight(0xffffff, 1);
        directionalLight1.position.set(5, 5, 5);
        scene.add(directionalLight1);

        const directionalLight2 = new THREE.DirectionalLight(0xffffff, 0.5);
        directionalLight2.position.set(-5, -5, -5);
        scene.add(directionalLight2);

        // Add axes helper
        const axesHelper = new THREE.AxesHelper(2);
        scene.add(axesHelper);

        // Add grid
        const gridHelper = new THREE.GridHelper(10, 10, 0x444444, 0x222222);
        scene.add(gridHelper);

        // Position camera
        const box = new THREE.Box3().setFromObject(mesh);
        const center = box.getCenter(new THREE.Vector3());
        const size = box.getSize(new THREE.Vector3());
        const maxDim = Math.max(size.x, size.y, size.z);
        const fov = camera.fov * (Math.PI / 180);
        let cameraZ = Math.abs(maxDim / 2 / Math.tan(fov / 2));
        cameraZ *= 2.5; // Add some padding

        camera.position.set(center.x + cameraZ * 0.5, center.y + cameraZ * 0.5, center.z + cameraZ);
        camera.lookAt(center);

        // Add controls
        const controls = new OrbitControls(camera, renderer.domElement);
        controls.target.copy(center);
        controls.enableDamping = true;
        controls.dampingFactor = 0.05;
        controls.update();

        // Handle window resize
        window.addEventListener('resize', () => {{
            camera.aspect = window.innerWidth / window.innerHeight;
            camera.updateProjectionMatrix();
            renderer.setSize(window.innerWidth, window.innerHeight);
        }});

        // Animation loop
        function animate() {{
            requestAnimationFrame(animate);
            controls.update();
            renderer.render(scene, camera);
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

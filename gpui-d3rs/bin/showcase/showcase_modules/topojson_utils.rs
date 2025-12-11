use d3rs::geo::GeoJsonGeometry;
use serde::Deserialize;


#[derive(Deserialize, Debug)]
pub struct Transform {
    pub scale: [f64; 2],
    pub translate: [f64; 2],
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type")]
pub enum GeometryObject {
    MultiPolygon { arcs: Vec<Vec<Vec<i32>>> },
    Polygon { arcs: Vec<Vec<i32>> },
    // Simplified for land-50m.json which we know is mainly MultiPolygon or Polygon
}

#[derive(Deserialize, Debug)]
pub struct Objects {
    pub land: GeometryObject,
}

#[derive(Deserialize, Debug)]
pub struct Topology {
    pub objects: Objects,
    pub arcs: Vec<Vec<[i32; 2]>>,
    pub transform: Transform,
}

pub fn parse_topojson(json_str: &str) -> Option<GeoJsonGeometry> {
    let topology: Topology = serde_json::from_str(json_str).ok()?;
    
    // Decode all arcs first
    let scale = topology.transform.scale;
    let translate = topology.transform.translate;
    
    let decoded_arcs: Vec<Vec<(f64, f64)>> = topology.arcs.iter().map(|arc| {
        let mut x = 0;
        let mut y = 0;
        let mut points = Vec::with_capacity(arc.len());
        for point in arc {
            x += point[0];
            y += point[1];
            let px = x as f64 * scale[0] + translate[0];
            let py = y as f64 * scale[1] + translate[1];
            points.push((px, py));
        }
        points
    }).collect();

    // Convert Geometry to GeoJsonGeometry
    match topology.objects.land {
        GeometryObject::MultiPolygon { arcs } => {
            let mut multi_polygon = Vec::new();
            for polygon_arcs in arcs {
                let mut polygon = Vec::new();
                for ring_arcs in polygon_arcs {
                    let mut ring = Vec::new();
                    for &arc_idx in &ring_arcs {
                        let arc = if arc_idx >= 0 {
                            &decoded_arcs[arc_idx as usize]
                        } else {
                            &decoded_arcs[(!arc_idx) as usize] // bitwise not for negative index
                        };
                        
                        if arc_idx < 0 {
                            // Reversed arc
                            // Note: We might need to handle duplicate points at joints, usually stitched.
                            // GeoJSON rings should be closed.
                            for p in arc.iter().rev() {
                                ring.push(*p);
                            }
                        } else {
                            for p in arc {
                                ring.push(*p);
                            }
                        }
                    }
                    // Clean up potential duplicate points at seams if naive stitching?
                    // Usually stitching involves dropping the first/last point of subsequent arcs to avoid dupes,
                    // but simple concatenation works for render paths often, though valid GeoJSON shouldn't have adjacent dups.
                    // For now, let's just push all. If visual artifacts, we fix.
                    
                    // Actually, standard is: consecutive arcs share a point.
                    // If we just concatenate, we get A->B, B->C. We will have B twice.
                    // We should probably skip the first point of subsequent arcs?
                    // Or relies on the fact that if we have a list of arcs, we stitch them.
                    
                    // Refined stitching:
                    let mut stitched_ring = Vec::new();
                    for (i, &arc_idx) in ring_arcs.iter().enumerate() {
                        let arc = if arc_idx >= 0 {
                            &decoded_arcs[arc_idx as usize]
                        } else {
                            &decoded_arcs[(!arc_idx) as usize]
                        };
                        
                        // If reversed, we iterate rev.
                        if arc_idx < 0 {
                            for (j, p) in arc.iter().rev().enumerate() {
                                if i > 0 && j == 0 { continue; } // Skip first point if not first arc (it's same as prev last)
                                stitched_ring.push(*p);
                            }
                        } else {
                            for (j, p) in arc.iter().enumerate() {
                                if i > 0 && j == 0 { continue; }
                                stitched_ring.push(*p);
                            }
                        }
                    }
                    polygon.push(stitched_ring);
                }
                multi_polygon.push(polygon);
            }
            Some(GeoJsonGeometry::MultiPolygon(multi_polygon))
        },
        GeometryObject::Polygon { arcs } => {
             // Handle single polygon same as above but one level less
             let mut polygon = Vec::new();
             for ring_arcs in arcs {
                let mut stitched_ring = Vec::new();
                for (i, &arc_idx) in ring_arcs.iter().enumerate() {
                    let arc = if arc_idx >= 0 {
                        &decoded_arcs[arc_idx as usize]
                    } else {
                        &decoded_arcs[(!arc_idx) as usize]
                    };
                    
                    if arc_idx < 0 {
                         for (j, p) in arc.iter().rev().enumerate() {
                            if i > 0 && j == 0 { continue; }
                            stitched_ring.push(*p);
                        }
                    } else {
                        for (j, p) in arc.iter().enumerate() {
                            if i > 0 && j == 0 { continue; }
                            stitched_ring.push(*p);
                        }
                    }
                }
                polygon.push(stitched_ring);
             }
             Some(GeoJsonGeometry::Polygon(polygon))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_land_50m() {
        // Path relative to this file
        let json = include_str!("../data/land-50m.json");
        let result = parse_topojson(json);
        assert!(result.is_some(), "Failed to parse land-50m.json");
    }
}

//! Read-only import of explicitly registered room geometry, never acoustic approval.
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub struct RoomGeometry {
    pub vertices: Vec<[f32; 3]>,
    pub indices: Vec<[u32; 3]>,
    pub provenance: String,
}

fn bounded_read(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let file = std::fs::File::open(path).map_err(|e| format!("Cannot open room input: {e}"))?;
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > limit {
        return Err("Room input exceeds preview size limit".into());
    }
    Ok(bytes)
}

fn child_path(parent: &Path, value: &str) -> Result<PathBuf, String> {
    let relative = Path::new(value);
    if relative.is_absolute() {
        return Err("Room mesh reference must be relative to its model".into());
    }
    let base = parent.canonicalize().map_err(|e| e.to_string())?;
    let path = base
        .join(relative)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if !path.starts_with(&base) {
        return Err("Room mesh reference escapes the model directory".into());
    }
    Ok(path)
}

impl RoomGeometry {
    pub fn load_model(path: &Path) -> Result<Self, String> {
        let bytes = bounded_read(path, 2 * 1024 * 1024)?;
        let model: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        if model["units"]["length"] != "metres" || model["coordinate_system"]["z"] != "upward" {
            return Err(
                "Room preview requires explicit metre units and saved +Z-up coordinates".into(),
            );
        }
        let evidence = &model["evidence"];
        let mesh = evidence["mesh"]
            .as_str()
            .ok_or("Room model has no mesh reference")?;
        let expected = evidence["mesh_sha256"]
            .as_str()
            .ok_or("Room model has no mesh digest")?;
        let mesh_path = child_path(
            path.parent().ok_or("Room model has no parent directory")?,
            mesh,
        )?;
        let mesh_bytes = bounded_read(&mesh_path, 16 * 1024 * 1024)?;
        let hash = Sha256::digest(&mesh_bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        if hash != expected {
            return Err("Room mesh digest disagrees with its saved model".into());
        }
        let text = std::str::from_utf8(&mesh_bytes).map_err(|e| e.to_string())?;
        let (vertices, indices) = parse_obj(text)?;
        let provenance = serde_json::json!({
            "role":"registered room geometry preview",
            "model_sha256":Sha256::digest(&bytes).iter().map(|byte| format!("{byte:02x}")).collect::<String>(),
            "mesh_sha256":hash,"units":"metres",
            "coordinate_system":model["coordinate_system"],
            "source_solver_approved":evidence["mesh_solver_approved"],
            "preview_grants_acoustic_approval":false,
            "transformation":"none; saved coordinates retained",
        })
        .to_string();
        Ok(Self {
            vertices,
            indices,
            provenance,
        })
    }
}

fn parse_obj(text: &str) -> Result<(Vec<[f32; 3]>, Vec<[u32; 3]>), String> {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    for (line_index, line) in text.lines().enumerate() {
        let mut fields = line
            .split('#')
            .next()
            .unwrap_or_default()
            .split_whitespace();
        match fields.next() {
            None | Some("o" | "g" | "s" | "vn" | "vt" | "usemtl" | "mtllib") => continue,
            Some("v") => {
                let values = fields
                    .map(str::parse::<f32>)
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|_| format!("Invalid OBJ vertex on line {}", line_index + 1))?;
                if values.len() != 3
                    || values.iter().any(|v| !v.is_finite())
                    || vertices.len() >= 100_000
                {
                    return Err("Room preview needs finite XYZ vertices (maximum 100,000)".into());
                }
                vertices.push([values[0], values[1], values[2]]);
            }
            Some("f") => {
                let face = fields
                    .map(|field| {
                        let value = field
                            .split('/')
                            .next()
                            .unwrap_or_default()
                            .parse::<i64>()
                            .map_err(|_| "Invalid OBJ face index")?;
                        let index = if value > 0 {
                            value - 1
                        } else {
                            vertices.len() as i64 + value
                        };
                        if value == 0 || index < 0 || index >= vertices.len() as i64 {
                            return Err("OBJ face references an unavailable vertex");
                        }
                        Ok(index as u32)
                    })
                    .collect::<Result<Vec<_>, &str>>()?;
                if face.len() != 3 || indices.len() >= 200_000 {
                    return Err(
                        "Room preview needs triangulated OBJ faces (maximum 200,000)".into(),
                    );
                }
                if face[0] == face[1] || face[1] == face[2] || face[0] == face[2] {
                    return Err("Room preview rejects repeated-index faces".into());
                }
                indices.push([face[0], face[1], face[2]]);
            }
            Some(_) => return Err("Room preview supports triangle OBJ geometry only".into()),
        }
    }
    if vertices.is_empty() || indices.is_empty() {
        return Err("Room mesh has no triangle geometry".into());
    }
    Ok((vertices, indices))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn obj_keeps_coordinates_and_rejects_invalid_geometry() {
        let (vertices, faces) = parse_obj("v 0 0 0\nv 1 0 0\nv 0 1 2\nf -3 -2 -1\n").unwrap();
        assert_eq!(vertices[2], [0., 1., 2.]);
        assert_eq!(faces, vec![[0, 1, 2]]);
        for text in ["v NaN 0 0", "v 0 0 0\nf 0 1 1", "v 0 0 0\nf 1 2 3", ""] {
            assert!(parse_obj(text).is_err());
        }
    }

    #[test]
    fn model_binds_units_coordinates_and_mesh_digest() {
        let dir = std::env::temp_dir().join(format!("sotf-room-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        let mesh = b"v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n";
        std::fs::write(dir.join("room.obj"), mesh).unwrap();
        let path = dir.join("room-model.json");
        let mut model = serde_json::json!({"units":{"length":"metres"},
            "coordinate_system":{"z":"upward"},
            "evidence":{"mesh":"room.obj","mesh_sha256":Sha256::digest(mesh).iter().map(|byte| format!("{byte:02x}")).collect::<String>()}});
        std::fs::write(&path, model.to_string()).unwrap();
        assert_eq!(RoomGeometry::load_model(&path).unwrap().indices.len(), 1);
        model["units"]["length"] = "millimetres".into();
        std::fs::write(&path, model.to_string()).unwrap();
        assert!(RoomGeometry::load_model(&path).is_err());
        model["units"]["length"] = "metres".into();
        model["evidence"]["mesh_sha256"] = "incorrect".into();
        std::fs::write(&path, model.to_string()).unwrap();
        assert!(RoomGeometry::load_model(&path).is_err());
        assert!(child_path(&dir, "../room.obj").is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
}

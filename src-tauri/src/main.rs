// Rufus para Mac — núcleo. Tudo é shell-out pra ferramentas nativas do macOS:
// diskutil (listar/desmontar/ejetar), dd (gravar), osascript (escolher arquivo + admin).
// cyrix: zero FFI IOKit `unsafe` — diskutil já entrega tudo em plist.
use serde::Serialize;
use std::process::Command;

#[derive(Serialize, Clone)]
struct Disk {
    id: String,   // ex: "disk4"
    name: String, // ex: "SanDisk Cruzer Blade"
    size: u64,    // bytes
}

/// Extrai os discos do plist do `diskutil list -plist`. Separado de list_disks pra ser testável sem shell.
fn parse_disks(bytes: &[u8]) -> Vec<Disk> {
    let v: plist::Value = match plist::from_bytes(bytes) {
        Ok(v) => v,
        Err(_) => return vec![],
    };
    v.as_dictionary()
        .and_then(|d| d.get("AllDisksAndPartitions"))
        .and_then(|a| a.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|d| {
                    let d = d.as_dictionary()?;
                    let id = d.get("DeviceIdentifier")?.as_string()?.to_string();
                    let size = d.get("Size").and_then(|s| s.as_unsigned_integer()).unwrap_or(0);
                    Some(Disk { name: id.clone(), id, size })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Nome amigável do disco (MediaName) via `diskutil info`. Best-effort.
fn disk_name(id: &str) -> Option<String> {
    let out = Command::new("diskutil").args(["info", "-plist", id]).output().ok()?;
    let v: plist::Value = plist::from_bytes(&out.stdout).ok()?;
    let d = v.as_dictionary()?;
    d.get("MediaName")
        .or_else(|| d.get("IORegistryEntryName"))
        .and_then(|x| x.as_string())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

#[tauri::command]
fn list_disks() -> Result<Vec<Disk>, String> {
    let out = Command::new("diskutil")
        .args(["list", "-plist", "external", "physical"])
        .output()
        .map_err(|e| e.to_string())?;
    let mut disks = parse_disks(&out.stdout);
    for d in &mut disks {
        if let Some(n) = disk_name(&d.id) {
            d.name = n;
        }
    }
    Ok(disks)
}

#[tauri::command]
fn pick_image(prompt: String) -> Option<String> {
    // cyrix: `choose file` nativo do AppleScript — sem plugin de diálogo, sem npm.
    // `prompt` vem da UI (idioma atual); escapa aspas/barra pra não quebrar o literal AppleScript.
    let safe = prompt.replace('\\', "\\\\").replace('"', "\\\"");
    let script = format!(r#"POSIX path of (choose file with prompt "{}")"#, safe);
    let out = Command::new("osascript").args(["-e", &script]).output().ok()?;
    if !out.status.success() {
        return None; // usuário cancelou
    }
    let p = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if p.is_empty() { None } else { Some(p) }
}

#[tauri::command]
fn flash(image: String, disk: String) -> Result<(), String> {
    // Validação no trust boundary: `disk` vem da nossa lista, mas garantimos o prefixo.
    if !disk.starts_with("disk") || disk.contains('/') {
        return Err("identificador de disco inválido".into());
    }
    if image.contains('"') {
        return Err("caminho com aspas duplas não suportado".into());
    }
    let dev = format!("/dev/{disk}");

    // Desmonta (não precisa de root pra disco externo do usuário).
    Command::new("diskutil")
        .args(["unmountDisk", &dev])
        .status()
        .map_err(|e| e.to_string())?;

    // dd como root via prompt de admin nativo. A senha é o gate de segurança.
    // cyrix: sem progresso ao vivo — osascript só retorna no fim.
    // upgrade: helper privilegiado (SMAppService) lançando dd + SIGINFO polling pra barra real.
    let q = shell_single_quote(&image);
    let script = format!(
        "do shell script \"/bin/dd if={} of=/dev/r{} bs=4m\" with administrator privileges",
        q, disk
    );
    let st = Command::new("osascript")
        .args(["-e", &script])
        .status()
        .map_err(|e| e.to_string())?;
    if !st.success() {
        return Err("dd falhou ou foi cancelado".into());
    }

    Command::new("diskutil").args(["eject", &dev]).status().ok();
    Ok(())
}

/// Aspas simples seguras pra shell (dentro do literal do AppleScript).
fn shell_single_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![list_disks, pick_image, flash])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o app");
}

#[cfg(test)]
mod tests {
    use super::{parse_disks, shell_single_quote};

    #[test]
    fn parses_one_external_disk() {
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>AllDisksAndPartitions</key><array><dict>
<key>DeviceIdentifier</key><string>disk4</string>
<key>Size</key><integer>15678312448</integer>
<key>Content</key><string>FDisk_partition_scheme</string>
</dict></array></dict></plist>"#;
        let d = parse_disks(xml);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].id, "disk4");
        assert_eq!(d[0].size, 15_678_312_448);
    }

    #[test]
    fn parses_empty_when_no_disks() {
        assert!(parse_disks(b"lixo nao-plist").is_empty());
    }

    #[test]
    fn quotes_paths_with_spaces_and_quotes() {
        assert_eq!(shell_single_quote("/a b/c.iso"), "'/a b/c.iso'");
        assert_eq!(shell_single_quote("/it's.iso"), "'/it'\\''s.iso'");
    }
}

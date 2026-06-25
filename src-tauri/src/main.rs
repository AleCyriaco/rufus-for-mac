// Rufus para Mac — núcleo. Tudo é shell-out pra ferramentas nativas do macOS:
// diskutil (listar/formatar/ejetar), hdiutil (montar ISO), rsync (copiar arquivos),
// wimlib (fatiar install.wim > 4GB), dd + osascript (gravar raw + prompt de admin).
// cyrix: zero FFI IOKit `unsafe` — diskutil já entrega tudo em plist.
use serde::Serialize;
use std::path::Path;
use std::process::Command;
use tauri::{Emitter, Window};

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
fn flash(window: Window, image: String, disk: String) -> Result<(), String> {
    // Validação no trust boundary: `disk` vem da nossa lista, mas garantimos o formato.
    if !disk.starts_with("disk") || disk.contains('/') {
        return Err("Invalid disk identifier".into());
    }
    if image.contains('"') {
        return Err("Image path with double quotes not supported".into());
    }
    // Emite o estágio atual pra UI (chave traduzida no front).
    let phase = |key: &str| {
        let _ = window.emit("flash-phase", key.to_string());
    };

    // Monta a ISO pra detectar Windows (tem sources/install.wim ou install.esd).
    phase("p_mount");
    let iso = hdiutil_mount(&image)?;
    let is_windows = Path::new(&format!("{iso}/sources/install.wim")).exists()
        || Path::new(&format!("{iso}/sources/install.esd")).exists();

    let result = if is_windows {
        flash_windows(&phase, &iso, &disk)
    } else {
        let _ = hdiutil_unmount(&iso); // dd lê o arquivo, não a ISO montada
        flash_dd(&phase, &image, &disk)
    };
    if is_windows {
        let _ = hdiutil_unmount(&iso);
    }
    result
}

/// ISO de Windows: USB em FAT32/MBR (UEFI removível) + cópia + split do install.wim. Sem root.
fn flash_windows(phase: &impl Fn(&str), iso_mount: &str, disk: &str) -> Result<(), String> {
    let dev = format!("/dev/{disk}");

    phase("p_format");
    let out = Command::new("diskutil")
        .args(["eraseDisk", "MS-DOS", "WIN11", "MBR", &dev])
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!("format failed: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }

    // MBR → a partição FAT é sempre diskNs1; pega o ponto de montagem que o diskutil criou.
    let usb = slice_mountpoint(&format!("{disk}s1")).ok_or("could not find the new FAT volume")?;

    // Copia tudo menos sources/install.wim (estoura o limite de 4GB do FAT32).
    // -rt: sem perms/owner/symlink — FAT não suporta e geraria exit code de atributo.
    phase("p_copy");
    let out = Command::new("rsync")
        .args([
            "-rt",
            "--exclude=sources/install.wim",
            &format!("{iso_mount}/"),
            &format!("{usb}/"),
        ])
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!("rsync failed: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }

    // Se há install.wim, fatia em .swm < 4GB (install.esd normalmente já coube na cópia).
    let wim = format!("{iso_mount}/sources/install.wim");
    if Path::new(&wim).exists() {
        let wimlib = find_wimlib().ok_or("wimlib not installed. Run: brew install wimlib")?;
        phase("p_split");
        let swm = format!("{usb}/sources/install.swm");
        let out = Command::new(wimlib)
            .args(["split", &wim, &swm, "3800"]) // 3800 MB < limite 4096 do FAT32
            .output()
            .map_err(|e| e.to_string())?;
        if !out.status.success() {
            return Err(format!("wimlib failed: {}", String::from_utf8_lossy(&out.stderr).trim()));
        }
    }

    phase("p_finish");
    Command::new("diskutil").args(["eject", &dev]).status().ok();
    Ok(())
}

/// Imagem genérica (Linux etc.): grava raw com dd como root (prompt de admin nativo).
fn flash_dd(phase: &impl Fn(&str), image: &str, disk: &str) -> Result<(), String> {
    let dev = format!("/dev/{disk}");
    phase("p_unmount");
    Command::new("diskutil")
        .args(["unmountDisk", "force", &dev])
        .status()
        .map_err(|e| e.to_string())?;

    // cyrix: sem progresso ao vivo — osascript admin só retorna no fim.
    // upgrade: helper privilegiado (SMAppService) lançando dd + SIGINFO polling pra barra real.
    phase("p_write");
    let q = shell_single_quote(image);
    let script = format!(
        "do shell script \"/bin/dd if={} of=/dev/r{} bs=4m\" with administrator privileges",
        q, disk
    );
    let out = Command::new("osascript")
        .args(["-e", &script])
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let low = err.to_lowercase();
        if low.contains("-128") || low.contains("cancel") {
            return Err("Canceled — admin password not entered.".into());
        }
        return Err(format!("dd failed: {}", err.trim()));
    }
    Command::new("diskutil").args(["eject", &dev]).status().ok();
    Ok(())
}

/// Monta a ISO (read-only, sem aparecer no Finder) e devolve o ponto de montagem.
fn hdiutil_mount(image: &str) -> Result<String, String> {
    let out = Command::new("hdiutil")
        .args(["mount", "-nobrowse", "-plist", image])
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!("hdiutil failed: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    let v: plist::Value = plist::from_bytes(&out.stdout).map_err(|e| e.to_string())?;
    v.as_dictionary()
        .and_then(|d| d.get("system-entities"))
        .and_then(|e| e.as_array())
        .and_then(|arr| {
            arr.iter().find_map(|e| {
                e.as_dictionary()?
                    .get("mount-point")?
                    .as_string()
                    .map(|s| s.to_string())
            })
        })
        .ok_or_else(|| "could not find ISO mount point".into())
}

fn hdiutil_unmount(mount: &str) -> Result<(), String> {
    Command::new("hdiutil")
        .args(["unmount", mount])
        .status()
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// MountPoint de uma slice (ex.: disk4s1) via diskutil info.
fn slice_mountpoint(slice: &str) -> Option<String> {
    let out = Command::new("diskutil").args(["info", "-plist", slice]).output().ok()?;
    let v: plist::Value = plist::from_bytes(&out.stdout).ok()?;
    v.as_dictionary()?
        .get("MountPoint")?
        .as_string()
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
}

/// Acha o wimlib mesmo sem o PATH do shell — apps GUI no macOS não herdam o PATH do brew.
fn find_wimlib() -> Option<String> {
    for p in ["/opt/homebrew/bin/wimlib-imagex", "/usr/local/bin/wimlib-imagex"] {
        if Path::new(p).exists() {
            return Some(p.to_string());
        }
    }
    let out = Command::new("/usr/bin/which").arg("wimlib-imagex").output().ok()?;
    if out.status.success() {
        let p = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !p.is_empty() {
            return Some(p);
        }
    }
    None
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

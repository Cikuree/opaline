fn main() {
    println!("cargo:rerun-if-env-changed=DISCORD_APP_ID");
    tauri_build::build()
}

fn main() {
    #[cfg(target_os = "windows")]
    {
        let ico = "../../assets/steamlens.ico";
        println!("cargo:rerun-if-changed={ico}");

        let mut res = winresource::WindowsResource::new();
        res.set_icon(ico);
        res.set("ProductName", "SteamLens");
        res.set("FileDescription", "Steam achievement and stats inspector");
        res.compile()
            .expect("failed to embed Windows resources (icon + metadata)");
    }
}

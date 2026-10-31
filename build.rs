fn main() {
    println!("cargo:rerun-if-changed=resources/berserker-rgb.ico");
    println!("cargo:rerun-if-changed=resources/berserker-rgb.manifest");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("resources/berserker-rgb.ico");
        resource.set_manifest_file("resources/berserker-rgb.manifest");
        resource.set("FileDescription", "Berserker RGB — contrôle clavier");
        resource.set("ProductName", "Berserker RGB");
        resource.set("CompanyName", "ByGG");
        resource.set("LegalCopyright", "Copyright (c) 2026 ByGG");
        resource.set("OriginalFilename", "berserker-rgb.exe");
        resource.set("FileVersion", "1.3.5.0");
        resource.set("ProductVersion", "1.3.5.0");
        resource.set(
            "Comments",
            "Contrôle RGB per-touche pour clavier Berserker BYGG (EVision V2)",
        );
        if let Err(error) = resource.compile() {
            panic!("compilation des ressources Windows impossible : {error}");
        }
    }
}

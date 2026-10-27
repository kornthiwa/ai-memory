fn main() {
    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/dormnet.ico");
        res.set("ProductName", "KIOIS Net");
        res.set("FileDescription", "KIOIS Net - dormitory internet auto login");
        res.set("CompanyName", "KIOIS");
        res.set("LegalCopyright", "Copyright KIOIS");
        if let Err(e) = res.compile() {
            println!("cargo:warning=winres failed: {e}");
        }
    }
}

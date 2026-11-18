// The report template is embedded with include_dir!, which can't tell cargo
// to rebuild when template files change. A build script can.
fn main() {
    println!("cargo:rerun-if-changed=assets/templates");
}

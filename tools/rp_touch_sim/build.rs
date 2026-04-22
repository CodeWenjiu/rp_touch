fn main() {
    slint_build::compile("../../app/ui/rp_touch.slint").expect("failed to compile rp_touch UI");
    println!("cargo:rerun-if-changed=../../app/ui/rp_touch.slint");
}

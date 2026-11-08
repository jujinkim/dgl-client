unsafe extern "C" {
    fn dgl_power_saving() -> bool;
}

pub fn saving() -> bool {
    unsafe { dgl_power_saving() }
}

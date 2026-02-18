#[repr(C)]
pub struct DeviceBuilder {
    builder: Box<astarte_device_sdk::builder::DeviceBuilder>,
}

#[unsafe(no_mangle)]
pub extern "C" fn builder() -> DeviceBuilder {
    DeviceBuilder {
        builder: Box::new(astarte_device_sdk::builder::DeviceBuilder::new()),
    }
}

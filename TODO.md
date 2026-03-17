# TODOs

## 1

Fix the hidapi dependency

```toml
# HID communication
# Use linux-native-basic-udev to avoid pkg-config dependency on libudev-dev
hidapi = { version = "2.6", default-features = false, features = ["linux-native-basic-udev"] }
```

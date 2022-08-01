# what_stream

a simple crate to lookup the current vscode theme

```rust
    use what_theme::VsCodeSettings;
    // this gets the current theme name
    let current = VsCodeSettings::unwrap_current_theme().unwrap()

    // this parses the extension cache
    let settings = VsCodeSettings::new().unwrap();
    // and this finds the theme by name
    let found_theme = settings.find(&current).unwrap();
```

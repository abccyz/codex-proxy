// macOS-specific menu bar handling
// Creates a minimal custom menu bar (About + Quit only)

#[cfg(target_os = "macos")]
pub fn setup_custom_menu_bar() {
    use objc::{class, msg_send, sel, sel_impl};
    use objc::runtime::Object;

    unsafe {
        // Get shared NSApplication
        let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
        
        // Create main menu
        let menu_class = class!(NSMenu);
        let menu: *mut Object = msg_send![menu_class, new];
        
        // Create app menu item
        let menu_item_class = class!(NSMenuItem);
        let app_menu_item: *mut Object = msg_send![menu_item_class, new];
        
        // Create app submenu
        let app_menu: *mut Object = msg_send![menu_class, new];
        
        // "About" item
        let about_item: *mut Object = msg_send![menu_item_class, new];
        let about_title: *mut Object = msg_send![class!(NSString), stringWithUTF8String: "About Codex Proxy\0".as_ptr()];
        let _: () = msg_send![about_item, setTitle: about_title];
        let _: () = msg_send![about_item, setAction: sel!(orderFrontStandardAboutPanel:)];
        let _: () = msg_send![app_menu, addItem: about_item];
        
        // Separator
        let separator: *mut Object = msg_send![menu_item_class, separatorItem];
        let _: () = msg_send![app_menu, addItem: separator];
        
        // "Quit" item
        let quit_item: *mut Object = msg_send![menu_item_class, new];
        let quit_title: *mut Object = msg_send![class!(NSString), stringWithUTF8String: "Quit Codex Proxy\0".as_ptr()];
        let _: () = msg_send![quit_item, setTitle: quit_title];
        let quit_key: *mut Object = msg_send![class!(NSString), stringWithUTF8String: "q\0".as_ptr()];
        let _: () = msg_send![quit_item, setKeyEquivalent: quit_key];
        let _: () = msg_send![quit_item, setAction: sel!(terminate:)];
        let _: () = msg_send![app_menu, addItem: quit_item];
        
        // Set submenu
        let _: () = msg_send![app_menu_item, setSubmenu: app_menu];
        let _: () = msg_send![menu, addItem: app_menu_item];
        
        // Set main menu
        let _: () = msg_send![app, setMainMenu: menu];
    }
}

#[cfg(not(target_os = "macos"))]
pub fn setup_custom_menu_bar() {
    // No-op on non-macOS platforms
}

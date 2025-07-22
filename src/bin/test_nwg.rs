/*!
    测试native-windows-gui是否正常工作
*/

extern crate native_windows_derive as nwd;
extern crate native_windows_gui as nwg;

use nwd::NwgUi;
use nwg::NativeUi;

#[derive(Default, NwgUi)]
pub struct TestApp {
    #[nwg_control(size: (300, 200), position: (300, 300), title: "NWG测试", flags: "WINDOW|VISIBLE")]
    #[nwg_events( OnWindowClose: [TestApp::close] )]
    window: nwg::Window,

    #[nwg_control(text: "Hello NWG!", h_align: nwg::HTextAlign::Center)]
    #[nwg_layout_item(layout: layout, row: 0, col: 0)]
    label: nwg::Label,

    #[nwg_control(text: "测试按钮")]
    #[nwg_layout_item(layout: layout, row: 1, col: 0)]
    #[nwg_events( OnButtonClick: [TestApp::test_click] )]
    button: nwg::Button,

    #[nwg_layout(parent: window, spacing: 1)]
    layout: nwg::GridLayout,
}

impl TestApp {
    fn close(&self) {
        nwg::stop_thread_dispatch();
    }

    fn test_click(&self) {
        nwg::modal_info_message(&self.window, "测试", "按钮点击成功！");
    }
}

fn main() {
    nwg::init().expect("Failed to init Native Windows GUI");
    nwg::Font::set_global_family("Segoe UI").expect("Failed to set default font");
    let _app = TestApp::build_ui(Default::default()).expect("Failed to build UI");
    nwg::dispatch_thread_events();
}

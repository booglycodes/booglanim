use tauri::AppHandle;

pub struct EncodeVideoSignal {
    pub app_handle : AppHandle<>, 
    pub path : String
}

pub struct UpdateMediaResourcesSignal;

pub struct DisplaySignal {
    pub playing : bool,
    pub frame : Option<usize>
}
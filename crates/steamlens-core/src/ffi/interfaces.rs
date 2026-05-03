use core::ffi::{c_char, c_void};

pub type HSteamPipe = i32;
pub type HSteamUser = i32;

#[repr(C)]
pub struct ISteamClient018 {
    pub create_steam_pipe: unsafe extern "C" fn(this: *mut c_void) -> HSteamPipe,
    pub release_steam_pipe: unsafe extern "C" fn(this: *mut c_void, pipe: HSteamPipe) -> bool,
    pub connect_to_global_user:
        unsafe extern "C" fn(this: *mut c_void, pipe: HSteamPipe) -> HSteamUser,
    pub create_local_user: unsafe extern "C" fn(
        this: *mut c_void,
        pipe: *mut HSteamPipe,
        account_type: i32,
    ) -> HSteamUser,
    pub release_user: unsafe extern "C" fn(this: *mut c_void, pipe: HSteamPipe, user: HSteamUser),
    pub get_isteam_user: unsafe extern "C" fn(
        this: *mut c_void,
        user: HSteamUser,
        pipe: HSteamPipe,
        version: *const c_char,
    ) -> *mut c_void,
    _reserved_06_get_isteam_game_server: usize,
    _reserved_07_set_local_ip_binding: usize,
    _reserved_08_get_isteam_friends: usize,
    _reserved_09_get_isteam_utils: usize,
    _reserved_10_get_isteam_matchmaking: usize,
    _reserved_11_get_isteam_matchmaking_servers: usize,
    _reserved_12_get_isteam_generic_interface: usize,
    _reserved_13_get_isteam_user_stats: usize,
    _reserved_14_get_isteam_game_server_stats: usize,
    _reserved_15_get_isteam_apps: usize,
    _reserved_16_get_isteam_networking: usize,
    _reserved_17_get_isteam_remote_storage: usize,
    _reserved_18_get_isteam_screenshots: usize,
    _reserved_19_get_isteam_game_search: usize,
    _reserved_20_run_frame: usize,
    _reserved_21_get_ipc_call_count: usize,
    _reserved_22_set_warning_message_hook: usize,
    _reserved_23_shutdown_if_all_pipes_closed: usize,
    _reserved_24_get_isteam_http: usize,
    _reserved_25_deprecated_get_isteam_unified_messages: usize,
    _reserved_26_get_isteam_controller: usize,
    _reserved_27_get_isteam_ugc: usize,
    _reserved_28_get_isteam_app_list: usize,
    _reserved_29_get_isteam_music: usize,
    _reserved_30_get_isteam_music_remote: usize,
    _reserved_31_get_isteam_html_surface: usize,
    _reserved_32_deprecated_set_post_api_result: usize,
    _reserved_33_deprecated_remove_post_api_result: usize,
    _reserved_34_set_check_callback_registered: usize,
    _reserved_35_get_isteam_inventory: usize,
    _reserved_36_get_isteam_video: usize,
    _reserved_37_get_isteam_parental_settings: usize,
    _reserved_38_get_isteam_input: usize,
    _reserved_39_get_isteam_parties: usize,
}

#[repr(C)]
pub struct ISteamUser012 {
    pub get_h_steam_user: unsafe extern "C" fn(this: *mut c_void) -> HSteamUser,
    pub logged_on: unsafe extern "C" fn(this: *mut c_void) -> bool,
    pub get_steam_id: unsafe extern "C" fn(this: *mut c_void) -> u64,
    _reserved_03_initiate_game_connection: usize,
    _reserved_04_terminate_game_connection: usize,
    _reserved_05_track_app_usage_event: usize,
    _reserved_06_get_user_data_folder: usize,
    _reserved_07_start_voice_recording: usize,
    _reserved_08_stop_voice_recording: usize,
    _reserved_09_get_compressed_voice: usize,
    _reserved_10_decompress_voice: usize,
    _reserved_11_get_auth_session_ticket: usize,
    _reserved_12_begin_auth_session: usize,
    _reserved_13_end_auth_session: usize,
    _reserved_14_cancel_auth_ticket: usize,
    _reserved_15_user_has_license_for_app: usize,
}

pub type CreateInterfaceFn =
    unsafe extern "C" fn(version: *const c_char, return_code: *mut i32) -> *mut c_void;

use core::ffi::{c_char, c_void};

pub type HSteamPipe = i32;
pub type HSteamUser = i32;

/// Wire-level callback message as written by Steam into the buffer passed to
/// `Steam_BGetCallback`. The layout matches the C-style struct that Steam
/// writes on the stack: 4-byte user handle, 4-byte callback id, pointer-sized
/// param pointer, 4-byte param byte count. `Pack = 1` in the canonical
/// reference means no implicit padding is inserted between fields; we must
/// declare this accordingly so Steam's write lines up with our reads.
#[repr(C, packed)]
pub struct CallbackMessage {
    pub user: HSteamUser,
    pub id: i32,
    pub param_ptr: *mut u8,
    pub param_size: i32,
}

pub type BGetCallbackFn =
    unsafe extern "C" fn(pipe: HSteamPipe, msg: *mut CallbackMessage, call: *mut i32) -> bool;

pub type FreeLastCallbackFn = unsafe extern "C" fn(pipe: HSteamPipe) -> bool;

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
    pub get_isteam_user_stats: unsafe extern "C" fn(
        this: *mut c_void,
        user: HSteamUser,
        pipe: HSteamPipe,
        version: *const c_char,
    ) -> *mut c_void,
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

/// Vtable layout for `ISteamUserStats013` as vended by
/// `GetISteamUserStats("STEAMUSERSTATS_INTERFACE_VERSION013")`.
///
/// Field order must match the canonical interface definition exactly —
/// Steam dispatches by vtable index (positional), so reordering fields
/// changes which method is called.
#[repr(C)]
pub struct ISteamUserStats013 {
    pub get_stat_float:
        unsafe extern "C" fn(this: *mut c_void, name: *const c_char, data: *mut f32) -> bool,
    pub get_stat_int:
        unsafe extern "C" fn(this: *mut c_void, name: *const c_char, data: *mut i32) -> bool,
    pub set_stat_float:
        unsafe extern "C" fn(this: *mut c_void, name: *const c_char, data: f32) -> bool,
    pub set_stat_int:
        unsafe extern "C" fn(this: *mut c_void, name: *const c_char, data: i32) -> bool,
    _reserved_04_update_avg_rate_stat: usize,
    pub get_achievement:
        unsafe extern "C" fn(this: *mut c_void, name: *const c_char, achieved: *mut bool) -> bool,
    pub set_achievement: unsafe extern "C" fn(this: *mut c_void, name: *const c_char) -> bool,
    pub clear_achievement: unsafe extern "C" fn(this: *mut c_void, name: *const c_char) -> bool,
    pub get_achievement_and_unlock_time: unsafe extern "C" fn(
        this: *mut c_void,
        name: *const c_char,
        achieved: *mut bool,
        unlock_time: *mut u32,
    ) -> bool,
    pub store_stats: unsafe extern "C" fn(this: *mut c_void) -> bool,
    pub get_achievement_icon: unsafe extern "C" fn(this: *mut c_void, name: *const c_char) -> i32,
    pub get_achievement_display_attribute: unsafe extern "C" fn(
        this: *mut c_void,
        name: *const c_char,
        key: *const c_char,
    ) -> *const c_char,
    pub indicate_achievement_progress: unsafe extern "C" fn(
        this: *mut c_void,
        name: *const c_char,
        current: u32,
        max: u32,
    ) -> bool,
    pub get_num_achievements: unsafe extern "C" fn(this: *mut c_void) -> u32,
    pub get_achievement_name: unsafe extern "C" fn(this: *mut c_void, index: u32) -> *const c_char,
    pub request_user_stats: unsafe extern "C" fn(this: *mut c_void, steam_id: u64) -> u64,
    pub get_user_stat_float: unsafe extern "C" fn(
        this: *mut c_void,
        steam_id: u64,
        name: *const c_char,
        data: *mut f32,
    ) -> bool,
    pub get_user_stat_int: unsafe extern "C" fn(
        this: *mut c_void,
        steam_id: u64,
        name: *const c_char,
        data: *mut i32,
    ) -> bool,
    _reserved_18_get_user_achievement: usize,
    _reserved_19_get_user_achievement_and_unlock_time: usize,
    pub reset_all_stats: unsafe extern "C" fn(this: *mut c_void, achievements_too: bool) -> bool,
    _reserved_21_find_or_create_leaderboard: usize,
    _reserved_22_find_leaderboard: usize,
    _reserved_23_get_leaderboard_name: usize,
    _reserved_24_get_leaderboard_entry_count: usize,
    _reserved_25_get_leaderboard_sort_method: usize,
    _reserved_26_get_leaderboard_display_type: usize,
    _reserved_27_download_leaderboard_entries: usize,
    _reserved_28_download_leaderboard_entries_for_users: usize,
    _reserved_29_get_downloaded_leaderboard_entry: usize,
    _reserved_30_upload_leaderboard_score: usize,
    _reserved_31_attach_leaderboard_ugc: usize,
    _reserved_32_get_number_of_current_players: usize,
    _reserved_33_request_global_achievement_percentages: usize,
    _reserved_34_get_most_achieved_achievement_info: usize,
    _reserved_35_get_next_most_achieved_achievement_info: usize,
    _reserved_36_get_achievement_achieved_percent: usize,
    _reserved_37_request_global_stats: usize,
    _reserved_38_get_global_stat_float: usize,
    _reserved_39_get_global_stat_integer: usize,
    _reserved_40_get_global_stat_history_float: usize,
    _reserved_41_get_global_stat_history_integer: usize,
    _reserved_42_get_achievement_progress_limits_float: usize,
    _reserved_43_get_achievement_progress_limits_integer: usize,
}

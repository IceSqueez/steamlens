use core::ffi::{c_char, c_void};

pub type HSteamPipe = i32;
pub type HSteamUser = i32;

/// `#[repr(C, packed)]` is load-bearing — Steam writes this struct
/// with `#pragma pack(1)`; any implicit padding desyncs reads.
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
    pub get_isteam_friends: unsafe extern "C" fn(
        this: *mut c_void,
        user: HSteamUser,
        pipe: HSteamPipe,
        version: *const c_char,
    ) -> *mut c_void,
    pub get_isteam_utils: unsafe extern "C" fn(
        this: *mut c_void,
        pipe: HSteamPipe,
        version: *const c_char,
    ) -> *mut c_void,
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
    pub get_isteam_apps: unsafe extern "C" fn(
        this: *mut c_void,
        user: HSteamUser,
        pipe: HSteamPipe,
        version: *const c_char,
    ) -> *mut c_void,
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
    pub get_user_data_folder:
        unsafe extern "C" fn(this: *mut c_void, buffer: *mut c_char, buffer_size: i32) -> bool,
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

/// `SteamUser023` vtable — field order is positional; slot order load-bearing.
#[repr(C)]
pub struct ISteamUser023 {
    _reserved_00_get_h_steam_user: usize,
    pub b_logged_on: unsafe extern "C" fn(this: *mut c_void) -> bool,
    _reserved_02_get_steam_id: usize,
    _reserved_03_initiate_game_connection_deprecated: usize,
    _reserved_04_terminate_game_connection_deprecated: usize,
    _reserved_05_track_app_usage_event: usize,
    _reserved_06_get_user_data_folder: usize,
    _reserved_07_start_voice_recording: usize,
    _reserved_08_stop_voice_recording: usize,
    _reserved_09_get_available_voice: usize,
    _reserved_10_get_voice: usize,
    _reserved_11_decompress_voice: usize,
    _reserved_12_get_voice_optimal_sample_rate: usize,
    _reserved_13_get_auth_session_ticket: usize,
    _reserved_14_get_auth_ticket_for_web_api: usize,
    _reserved_15_begin_auth_session: usize,
    _reserved_16_end_auth_session: usize,
    _reserved_17_cancel_auth_ticket: usize,
    _reserved_18_user_has_license_for_app: usize,
    _reserved_19_b_is_behind_nat: usize,
    _reserved_20_advertise_game: usize,
    _reserved_21_request_encrypted_app_ticket: usize,
    _reserved_22_get_encrypted_app_ticket: usize,
    _reserved_23_get_game_badge_level: usize,
    pub get_player_steam_level: unsafe extern "C" fn(this: *mut c_void) -> i32,
}

pub type CreateInterfaceFn =
    unsafe extern "C" fn(version: *const c_char, return_code: *mut i32) -> *mut c_void;

/// `STEAMUSERSTATS_INTERFACE_VERSION013` vtable — field order is
/// load-bearing; positional dispatch by Steam.
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
    pub request_global_achievement_percentages: unsafe extern "C" fn(this: *mut c_void) -> u64,
    _reserved_34_get_most_achieved_achievement_info: usize,
    _reserved_35_get_next_most_achieved_achievement_info: usize,
    pub get_achievement_achieved_percent:
        unsafe extern "C" fn(this: *mut c_void, name: *const c_char, percent: *mut f32) -> bool,
    _reserved_37_request_global_stats: usize,
    _reserved_38_get_global_stat_float: usize,
    _reserved_39_get_global_stat_integer: usize,
    _reserved_40_get_global_stat_history_float: usize,
    _reserved_41_get_global_stat_history_integer: usize,
    _reserved_42_get_achievement_progress_limits_float: usize,
    _reserved_43_get_achievement_progress_limits_integer: usize,
}

/// `SteamFriends009` vtable — field order is load-bearing.
#[repr(C)]
pub struct ISteamFriends009 {
    pub get_persona_name: unsafe extern "C" fn(this: *mut c_void) -> *const c_char,
    _reserved_01_set_persona_name: usize,
    _reserved_02_get_persona_state: usize,
    _reserved_03_get_friend_count: usize,
    _reserved_04_get_friend_by_index: usize,
    _reserved_05_get_friend_relationship: usize,
    _reserved_06_get_friend_persona_state: usize,
    _reserved_07_get_friend_persona_name: usize,
    _reserved_08_get_friend_game_played: usize,
    _reserved_09_get_friend_persona_name_history: usize,
    _reserved_10_has_friend: usize,
    _reserved_11_get_clan_count: usize,
    _reserved_12_get_clan_by_index: usize,
    _reserved_13_get_clan_name: usize,
    _reserved_14_get_clan_tag: usize,
    _reserved_15_get_friend_count_from_source: usize,
    _reserved_16_get_friend_from_source_by_index: usize,
    _reserved_17_is_user_in_source: usize,
    _reserved_18_set_in_game_voice_speaking: usize,
    _reserved_19_activate_game_overlay: usize,
    _reserved_20_activate_game_overlay_to_user: usize,
    _reserved_21_activate_game_overlay_to_web_page: usize,
    _reserved_22_activate_game_overlay_to_store: usize,
    _reserved_23_set_played_with: usize,
    _reserved_24_activate_game_overlay_invite_dialog: usize,
    _reserved_25_get_small_friend_avatar: usize,
    pub get_medium_friend_avatar: unsafe extern "C" fn(this: *mut c_void, steam_id: u64) -> i32,
    _reserved_27_get_large_friend_avatar: usize,
}

#[repr(C)]
pub struct ISteamApps001 {
    pub get_app_data: unsafe extern "C" fn(
        this: *mut c_void,
        app_id: u32,
        key: *const c_char,
        value: *mut c_char,
        value_length: i32,
    ) -> i32,
}

#[repr(C)]
pub struct ISteamApps008 {
    pub is_subscribed: unsafe extern "C" fn(this: *mut c_void) -> bool,
    _reserved_01_is_low_violence: usize,
    _reserved_02_is_cybercafe: usize,
    _reserved_03_is_vac_banned: usize,
    _reserved_04_get_current_game_language: usize,
    _reserved_05_get_available_game_languages: usize,
    pub is_subscribed_app: unsafe extern "C" fn(this: *mut c_void, app_id: u32) -> bool,
    _reserved_07_is_dlc_installed: usize,
    _reserved_08_get_earliest_purchase_unix_time: usize,
    _reserved_09_is_subscribed_from_free_weekend: usize,
    _reserved_10_get_dlc_count: usize,
    _reserved_11_get_dlc_data_by_index: usize,
    _reserved_12_install_dlc: usize,
    _reserved_13_uninstall_dlc: usize,
    _reserved_14_request_app_proof_of_purchase_key: usize,
    _reserved_15_get_current_beta_name: usize,
    _reserved_16_mark_content_corrupt: usize,
    _reserved_17_get_installed_depots: usize,
    _reserved_18_get_app_install_dir: usize,
    pub is_app_installed: unsafe extern "C" fn(this: *mut c_void, app_id: u32) -> bool,
    _reserved_20_get_app_owner: usize,
    _reserved_21_get_launch_query_param: usize,
    _reserved_22_get_dlc_download_progress: usize,
    _reserved_23_get_app_build_id: usize,
    _reserved_24_request_all_proof_of_purchase_keys: usize,
    _reserved_25_get_file_details: usize,
    _reserved_26_get_launch_command_line: usize,
    _reserved_27_is_subscribed_from_family_sharing: usize,
}

/// `SteamUtils005` vtable — field order is load-bearing.
#[repr(C)]
pub struct ISteamUtils005 {
    _reserved_00_get_seconds_since_app_active: usize,
    _reserved_01_get_seconds_since_computer_active: usize,
    _reserved_02_get_connected_universe: usize,
    _reserved_03_get_server_real_time: usize,
    _reserved_04_get_ip_country: usize,
    pub get_image_size: unsafe extern "C" fn(
        this: *mut c_void,
        handle: i32,
        width: *mut u32,
        height: *mut u32,
    ) -> bool,
    pub get_image_rgba:
        unsafe extern "C" fn(this: *mut c_void, handle: i32, dest: *mut u8, dest_size: i32) -> bool,
    _reserved_07_get_cser_ip_port: usize,
    _reserved_08_get_current_battery_power: usize,
    _reserved_09_get_app_id: usize,
    _reserved_10_set_overlay_notification_position: usize,
    pub is_api_call_completed:
        unsafe extern "C" fn(this: *mut c_void, handle: u64, failed: *mut bool) -> bool,
    _reserved_12_get_api_call_failure_reason: usize,
    pub get_api_call_result: unsafe extern "C" fn(
        this: *mut c_void,
        handle: u64,
        callback: *mut c_void,
        callback_size: i32,
        callback_expected: i32,
        failed: *mut bool,
    ) -> bool,
    _reserved_14_run_frame: usize,
    _reserved_15_get_ipc_call_count: usize,
    _reserved_16_set_warning_message_hook: usize,
    _reserved_17_is_overlay_enabled: usize,
    _reserved_18_overlay_needs_present: usize,
}

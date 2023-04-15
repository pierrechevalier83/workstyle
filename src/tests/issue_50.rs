use crate::config::Config;
use crate::pretty_window;
use crate::window_manager::Window;

const CONFIG_ISSUE_50: &str = "# Config for workstyle
#
# Format:
# \"pattern\" = \"icon\"
#
# The pattern will be used to match against the application name, class_id or WM_CLASS.
# The icon will be used to represent that application.
#
# Note if multiple patterns are present in the same application name,
# precedence is given in order of apparition in this file.

## partials
'/GitHub/' = ''
'/GitLab/' = ''
'/NVIM ?\\w*/' = ''
'/npm/' = ''
'/node/' = ''
'/yarn/' = ''
'/Stack Overflow/' = ''

## browsers
'google-chrome' = ''
'Google-chrome' = ''
'Google-chrome-unstable' = ''
'google-chrome-unstable' = ''
'Google-chrome-beta' = ''
'google-chrome-beta' = ''
'chromium' = ''
'firefox' = ''
'firefoxdeveloperedition' = ''

## default applications
'foot' = ''
'/foot/' = ''
'floating_shell' = ''
'pcmanfm' = ''
'nemo' = ''
'pamac-manager' = ''
'/Bluetooth/' = ''
'file-roller' = ''
'swappy' = ''
'org.kde.okular' = ''
'evince' = ''

## email
'Thunderbird' = ''
'thunderbird' = ''
'evolution' = ''
'kmail' = ''

## ide
'code' = '﬏'
'Code' = '﬏'
'/- Visual Studio Code/' = '﬏'
'/IntelliJ/' = ''
'code-url-handler' = '﬏'
'sublime_text' = ''

# messenger
'whatsapp-for-linux' = ''
'Slack' = ''
'/Telegram/' = ''
'/Microsoft Teams/' = ''
'Signal' = ''

## auth
'polkit-gnome-authentication-agent-1' = ''
'Keybase' = ''

## additional applications
'balena-etcher' = ''
'Steam' = ''
'vlc' = '嗢'
'org.qbittorrent.qBittorrent' = ''
'transmission-gtk' = ''
'Insomnia' = ''
'Bitwarden' = ''
'Spotify' = ''
'YouTube Music' = 'ﱘ'
'alacritty' = ''
'kitty' = ''
'font-manager' = ''
'lutris' = ''
'/Wine/' = ''
'Arctype' = ''
'Around' = ''

[other]
fallback_icon = ''
deduplicate_icons = true";

#[test]
fn test_pretty_window() {
    let w = Window {
        name: Some("Icons Icon | Font Awesome - Chromium".to_string()),
        app_id: None,
        window_properties_class: Some("chromium".to_string()),
    };
    let c = Config::from_str(CONFIG_ISSUE_50).unwrap();
    assert_eq!("", pretty_window(&c, &w));
}

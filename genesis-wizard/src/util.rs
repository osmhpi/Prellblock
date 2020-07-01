use dialoguer::{theme::Theme, Confirm, Input};
use std::{fs, path::Path};

pub(super) fn handle_set_path(
    theme: &'_ dyn Theme,
    reason: &str,
    default_directory: &str,
) -> String {
    loop {
        let path = Input::<String>::with_theme(theme)
            .with_prompt(format!("Please enter a folder to store the {}:", reason))
            .default(default_directory.to_owned())
            .interact()
            .unwrap();
        if Path::new(&path).exists() {
            return path;
        } else {
            let confirmation = Confirm::with_theme(theme)
                .with_prompt("The specified Path does not exist yet. Do you wish to create it?")
                .show_default(true)
                .default(false)
                .interact()
                .unwrap();
            if confirmation {
                match fs::create_dir_all(path.clone()) {
                    Ok(_) => {}
                    Err(err) => println!("Error while creating folder: {}", err),
                }
                return path;
            }
        }
    }
}

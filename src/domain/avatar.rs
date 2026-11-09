//! Profile pictures for people who sign up with an email address. Google
//! hands us a photo; an email sign-up gets one of these farm animals instead.

/// The animals, each drawn in every one of `COLORS`.
pub const ANIMALS: [&str; 4] = ["cow", "pig", "sheep", "chicken"];
/// Colorways, matching the file names under `public/images/avatars/`.
pub const COLORS: [&str; 4] = ["berry", "mint", "sky", "sunny"];

/// Public URL of one avatar, as served from `public/` under `/assets`.
pub fn url(animal: &str, color: &str) -> String {
    format!("/assets/images/avatars/{animal}-{color}.svg")
}

/// A randomly chosen avatar for a new email sign-up.
pub fn random() -> String {
    use rand::RngExt;
    let mut rng = rand::rng();
    let animal = ANIMALS[rng.random_range(0..ANIMALS.len())];
    let color = COLORS[rng.random_range(0..COLORS.len())];
    url(animal, color)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn all() -> Vec<String> {
        ANIMALS
            .iter()
            .flat_map(|animal| COLORS.iter().map(move |color| url(animal, color)))
            .collect()
    }

    #[test]
    fn every_avatar_has_a_file_behind_it() {
        for avatar in all() {
            let path = avatar.replacen("/assets/", "public/", 1);
            assert!(
                Path::new(env!("CARGO_MANIFEST_DIR")).join(&path).is_file(),
                "{path} is missing"
            );
        }
    }

    #[test]
    fn random_picks_one_of_the_avatars() {
        let avatars = all();
        for _ in 0..50 {
            assert!(avatars.contains(&random()));
        }
    }
}

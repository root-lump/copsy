use std::fmt;

use console::{Style, strip_ansi_codes, style};
use dialoguer::theme::ColorfulTheme;
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;

// ColorfulTheme renders active items char-by-char with active_item_style,
// which destroys ANSI codes embedded in items. This theme strips ANSI from
// active items first, then applies a uniform green+bold style.
pub struct CopsyTheme {
    inner: ColorfulTheme,
    active_style: Style,
}

impl CopsyTheme {
    pub fn new() -> Self {
        Self {
            inner: ColorfulTheme {
                active_item_prefix: style("❯".to_string()).for_stderr().green(),
                ..ColorfulTheme::default()
            },
            active_style: Style::new().for_stderr().green().bold(),
        }
    }
}

impl dialoguer::theme::Theme for CopsyTheme {
    fn format_fuzzy_select_prompt_item(
        &self,
        f: &mut dyn fmt::Write,
        text: &str,
        active: bool,
        highlight_matches: bool,
        matcher: &SkimMatcherV2,
        search_term: &str,
    ) -> fmt::Result {
        write!(
            f,
            "{} ",
            if active {
                &self.inner.active_item_prefix
            } else {
                &self.inner.inactive_item_prefix
            }
        )?;

        if active {
            // Strip ANSI so active_style doesn't collide with embedded colors
            let plain = strip_ansi_codes(text);
            if highlight_matches
                && let Some((_score, indices)) = matcher.fuzzy_indices(&plain, search_term)
            {
                for (idx, c) in plain.chars().enumerate() {
                    if indices.contains(&idx) {
                        write!(
                            f,
                            "{}",
                            self.active_style
                                .apply_to(self.inner.fuzzy_match_highlight_style.apply_to(c))
                        )?;
                    } else {
                        write!(f, "{}", self.active_style.apply_to(c))?;
                    }
                }
                return Ok(());
            }
            write!(f, "{}", self.active_style.apply_to(plain))
        // Inactive: preserve original ANSI colors from items
        } else if highlight_matches
            && let Some((_score, indices)) = matcher.fuzzy_indices(text, search_term)
        {
            for (idx, c) in text.chars().enumerate() {
                if indices.contains(&idx) {
                    write!(f, "{}", self.inner.fuzzy_match_highlight_style.apply_to(c))?;
                } else {
                    write!(f, "{}", c)?;
                }
            }
            Ok(())
        } else {
            write!(f, "{}", text)
        }
    }

    fn format_fuzzy_select_prompt(
        &self,
        f: &mut dyn fmt::Write,
        prompt: &str,
        search_term: &str,
        bytes_pos: usize,
    ) -> fmt::Result {
        self.inner
            .format_fuzzy_select_prompt(f, prompt, search_term, bytes_pos)
    }
}

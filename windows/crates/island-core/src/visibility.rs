//! Presentation is orthogonal to the two independent reasons to hide the window.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Presentation {
    #[default]
    Normal,
    Concentration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VisibleForm {
    Hidden,
    Normal,
    Concentration,
}

#[derive(Default, Debug)]
pub struct Visibility {
    pub manual: bool,
    pub fullscreen: bool,
    pub presentation: Presentation,
}

impl Visibility {
    pub fn hidden(&self) -> bool {
        self.manual || self.fullscreen
    }

    pub fn form(&self) -> VisibleForm {
        if self.hidden() {
            VisibleForm::Hidden
        } else {
            match self.presentation {
                Presentation::Normal => VisibleForm::Normal,
                Presentation::Concentration => VisibleForm::Concentration,
            }
        }
    }

    pub fn restore_normal(&mut self) {
        self.manual = false;
        self.presentation = Presentation::Normal;
        // Fullscreen is controlled only by the environment, never by a restore action.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_visibility_combination_has_one_form() {
        for manual in [false, true] {
            for fullscreen in [false, true] {
                for presentation in [Presentation::Normal, Presentation::Concentration] {
                    let v = Visibility {
                        manual,
                        fullscreen,
                        presentation,
                    };
                    assert_eq!(v.form() == VisibleForm::Hidden, manual || fullscreen);
                    if !v.hidden() {
                        assert_eq!(
                            v.form() == VisibleForm::Concentration,
                            presentation == Presentation::Concentration
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn fullscreen_and_manual_hide_preserve_presentation_until_explicit_restore() {
        let mut v = Visibility {
            presentation: Presentation::Concentration,
            manual: true,
            fullscreen: true,
        };
        v.fullscreen = false;
        assert_eq!(v.form(), VisibleForm::Hidden);
        v.manual = false;
        assert_eq!(v.form(), VisibleForm::Concentration);
        v.fullscreen = true;
        v.restore_normal();
        assert_eq!(v.form(), VisibleForm::Hidden);
        v.fullscreen = false;
        assert_eq!(v.form(), VisibleForm::Normal);
    }
}

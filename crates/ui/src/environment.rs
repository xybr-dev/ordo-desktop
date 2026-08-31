#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PreparationAppearance {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum InterfaceScale {
    #[default]
    Percent100,
    Percent125,
    Percent150,
    Percent175,
    Percent200,
}

impl InterfaceScale {
    pub const fn percent(self) -> u16 {
        match self {
            Self::Percent100 => 100,
            Self::Percent125 => 125,
            Self::Percent150 => 150,
            Self::Percent175 => 175,
            Self::Percent200 => 200,
        }
    }

    pub fn apply(self, value: f32) -> f32 {
        value * f32::from(self.percent()) / 100.0
    }

    const fn increase(self) -> Self {
        match self {
            Self::Percent100 => Self::Percent125,
            Self::Percent125 => Self::Percent150,
            Self::Percent150 => Self::Percent175,
            Self::Percent175 | Self::Percent200 => Self::Percent200,
        }
    }

    const fn decrease(self) -> Self {
        match self {
            Self::Percent100 | Self::Percent125 => Self::Percent100,
            Self::Percent150 => Self::Percent125,
            Self::Percent175 => Self::Percent150,
            Self::Percent200 => Self::Percent175,
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "100" => Some(Self::Percent100),
            "125" => Some(Self::Percent125),
            "150" => Some(Self::Percent150),
            "175" => Some(Self::Percent175),
            "200" => Some(Self::Percent200),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct UiPreferences {
    pub appearance: PreparationAppearance,
    pub scale: InterfaceScale,
}

impl UiPreferences {
    pub fn encode(self) -> String {
        let appearance = match self.appearance {
            PreparationAppearance::System => "system",
            PreparationAppearance::Light => "light",
            PreparationAppearance::Dark => "dark",
        };
        format!("appearance={appearance}\nscale={}\n", self.scale.percent())
    }

    pub fn decode(contents: &str) -> Result<Self, &'static str> {
        let mut appearance = None;
        let mut scale = None;
        for line in contents.lines() {
            let (name, value) = line.split_once('=').ok_or("invalid preference line")?;
            match name {
                "appearance" if appearance.is_none() => {
                    appearance = Some(match value {
                        "system" => PreparationAppearance::System,
                        "light" => PreparationAppearance::Light,
                        "dark" => PreparationAppearance::Dark,
                        _ => return Err("invalid appearance preference"),
                    });
                }
                "scale" if scale.is_none() => {
                    scale = Some(InterfaceScale::parse(value).ok_or("invalid scale preference")?);
                }
                "appearance" | "scale" => return Err("duplicate preference"),
                _ => return Err("unknown preference"),
            }
        }
        Ok(Self {
            appearance: appearance.ok_or("missing appearance preference")?,
            scale: scale.ok_or("missing scale preference")?,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolvedAppearance {
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum OperatorMode {
    #[default]
    Preparation,
    Presenter,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnvironmentChange {
    SetPreferences(UiPreferences),
    SetAppearance(PreparationAppearance),
    SetScale(InterfaceScale),
    IncreaseScale,
    DecreaseScale,
    ResetScale,
    EnterPresenter,
    LeavePresenter,
    SystemAppearanceChanged(ResolvedAppearance),
    SetHighContrast(bool),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UiEnvironment {
    preferences: UiPreferences,
    system_appearance: ResolvedAppearance,
    mode: OperatorMode,
    high_contrast: bool,
}

impl UiEnvironment {
    pub fn new(preferences: UiPreferences, system_appearance: ResolvedAppearance) -> Self {
        Self {
            preferences,
            system_appearance,
            mode: OperatorMode::Preparation,
            high_contrast: false,
        }
    }

    /// Whether a platform high-contrast mode owns focus and highlight
    /// indication. `DESIGN.md` requires Ordo to stand aside there, and the
    /// pinned GPUI revision reports no such mode, so this stays an operator
    /// escape hatch rather than something Ordo can detect.
    pub fn high_contrast(self) -> bool {
        self.high_contrast
    }

    pub fn preferences(self) -> UiPreferences {
        self.preferences
    }

    pub fn mode(self) -> OperatorMode {
        self.mode
    }

    pub fn appearance(self) -> ResolvedAppearance {
        if self.mode == OperatorMode::Presenter {
            return ResolvedAppearance::Dark;
        }
        match self.preferences.appearance {
            PreparationAppearance::System => self.system_appearance,
            PreparationAppearance::Light => ResolvedAppearance::Light,
            PreparationAppearance::Dark => ResolvedAppearance::Dark,
        }
    }

    pub fn apply(&mut self, change: EnvironmentChange) -> bool {
        let before = *self;
        match change {
            EnvironmentChange::SetPreferences(preferences) => self.preferences = preferences,
            EnvironmentChange::SetAppearance(appearance) => {
                self.preferences.appearance = appearance
            }
            EnvironmentChange::SetScale(scale) => self.preferences.scale = scale,
            EnvironmentChange::IncreaseScale => {
                self.preferences.scale = self.preferences.scale.increase();
            }
            EnvironmentChange::DecreaseScale => {
                self.preferences.scale = self.preferences.scale.decrease();
            }
            EnvironmentChange::ResetScale => self.preferences.scale = InterfaceScale::Percent100,
            EnvironmentChange::EnterPresenter => self.mode = OperatorMode::Presenter,
            EnvironmentChange::LeavePresenter => self.mode = OperatorMode::Preparation,
            EnvironmentChange::SystemAppearanceChanged(appearance) => {
                self.system_appearance = appearance;
            }
            EnvironmentChange::SetHighContrast(high_contrast) => {
                self.high_contrast = high_contrast;
            }
        }
        *self != before
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preferences_round_trip_and_reject_unsupported_values() {
        let preferences = UiPreferences {
            appearance: PreparationAppearance::Dark,
            scale: InterfaceScale::Percent175,
        };
        assert_eq!(
            UiPreferences::decode(&preferences.encode()),
            Ok(preferences)
        );
        assert!(UiPreferences::decode("appearance=dark\nscale=110\n").is_err());
    }

    #[test]
    fn presenter_override_does_not_replace_preparation_preference() {
        let mut environment = UiEnvironment::new(
            UiPreferences {
                appearance: PreparationAppearance::Light,
                scale: InterfaceScale::Percent100,
            },
            ResolvedAppearance::Dark,
        );

        environment.apply(EnvironmentChange::EnterPresenter);
        assert_eq!(environment.appearance(), ResolvedAppearance::Dark);
        environment.apply(EnvironmentChange::LeavePresenter);

        assert_eq!(environment.appearance(), ResolvedAppearance::Light);
        assert_eq!(
            environment.preferences().appearance,
            PreparationAppearance::Light
        );
    }

    #[test]
    fn system_changes_are_live_and_scale_clamps() {
        let mut environment =
            UiEnvironment::new(UiPreferences::default(), ResolvedAppearance::Light);

        environment.apply(EnvironmentChange::SystemAppearanceChanged(
            ResolvedAppearance::Dark,
        ));
        assert_eq!(environment.appearance(), ResolvedAppearance::Dark);
        for _ in 0..10 {
            environment.apply(EnvironmentChange::IncreaseScale);
        }
        assert_eq!(environment.preferences().scale, InterfaceScale::Percent200);
        for _ in 0..10 {
            environment.apply(EnvironmentChange::DecreaseScale);
        }
        assert_eq!(environment.preferences().scale, InterfaceScale::Percent100);
    }
}

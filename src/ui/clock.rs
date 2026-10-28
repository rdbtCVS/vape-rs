#[derive(Clone, Copy, Debug)]
pub struct ClockTime {
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub day: u8,
    pub month: u8,
    pub weekday: u8,
    pub american_date: bool,
}
impl Default for ClockTime {
    fn default() -> Self {
        Self {
            hour: 0,
            minute: 0,
            second: 0,
            day: 1,
            month: 1,
            weekday: 0,
            american_date: false,
        }
    }
}
impl ClockTime {
    pub fn hour_text(self, twenty_four: bool) -> String {
        format!(
            "{:02}",
            if twenty_four {
                self.hour
            } else {
                match self.hour % 12 {
                    0 => 12,
                    h => h,
                }
            }
        )
    }
    pub fn minute_text(self) -> String {
        format!("{:02}", self.minute)
    }
    pub fn meridiem(self) -> &'static str {
        if self.hour >= 12 { "pm" } else { "am" }
    }
    pub fn weekday_text(self) -> &'static str {
        ["mon", "tue", "wed", "thu", "fri", "sat", "sun"][self.weekday.min(6) as usize]
    }
    pub fn date(self, spaces: bool) -> String {
        let (a, b) = if self.american_date {
            (self.month, self.day)
        } else {
            (self.day, self.month)
        };
        if spaces {
            format!("{a:02} / {b:02}")
        } else {
            format!("{a:02}/{b:02}")
        }
    }
}

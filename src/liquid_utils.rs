use chrono::NaiveDateTime;

pub mod partials;


pub trait CustomFormat {
    fn to_env_string(&self, format: &str) -> String;
}
impl CustomFormat for NaiveDateTime {
    fn to_env_string(&self, format: &str) -> String {
        self.format(format).to_string()
    }
}
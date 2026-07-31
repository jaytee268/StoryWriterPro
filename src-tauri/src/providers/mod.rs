pub trait AiProvider {
    fn id(&self) -> &str;
    fn display_name(&self) -> &str;
}
pub struct MockProvider;
impl AiProvider for MockProvider {
    fn id(&self) -> &str {
        "mock"
    }
    fn display_name(&self) -> &str {
        "Mock Provider"
    }
}

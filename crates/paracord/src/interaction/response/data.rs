use serenity::builder::CreateInteractionResponseData;

/// A trait for values that can be built into a [`serenity`] interaction
/// response
pub trait ResponseData<'a> {
    /// Apply the values from `self` to the given response builder
    fn build_response_data<'b>(
        self,
        data: &'b mut CreateInteractionResponseData<'a>,
    ) -> &'b mut CreateInteractionResponseData<'a>;
}

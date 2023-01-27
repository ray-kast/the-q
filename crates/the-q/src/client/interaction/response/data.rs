use serenity::builder::CreateInteractionResponseData;

pub trait ResponseData<'a> {
    fn build_response_data<'b>(
        self,
        data: &'b mut CreateInteractionResponseData<'a>,
    ) -> &'b mut CreateInteractionResponseData<'a>;
}

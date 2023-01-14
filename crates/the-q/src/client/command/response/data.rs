use serenity::builder::CreateInteractionResponseData;

pub trait ResponseData {
    fn build_response_data<'a, 'b>(
        self,
        data: &'a mut CreateInteractionResponseData<'b>,
    ) -> &'a mut CreateInteractionResponseData<'b>;
}

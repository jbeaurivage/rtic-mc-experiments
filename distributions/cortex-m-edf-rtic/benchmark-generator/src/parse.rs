use syn::{
    Error, LitInt, Token, bracketed,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
};

pub(crate) struct Args {
    tasks_per_priority: LitInt,
    task_delay: LitInt,
    deadline_timings: Vec<LitInt>,
}

impl Parse for Args {
    fn parse(input: ParseStream) -> Result<Self, Error> {
        let tasks_per_priority: LitInt = input.parse()?;
        input.parse::<Token![,]>()?;

        let task_delay: LitInt = input.parse()?;
        input.parse::<Token![,]>()?;

        // Parse [10, 11, 12]
        let content;
        bracketed!(content in input);

        let values: Punctuated<LitInt, Token![,]> =
            content.parse_terminated(LitInt::parse, Token![,])?;

        Ok(Args {
            tasks_per_priority,
            task_delay,
            deadline_timings: values.into_iter().collect(),
        })
    }
}

impl TryFrom<Args> for Settings {
    type Error = Box<dyn std::error::Error>;

    fn try_from(args: Args) -> Result<Self, Self::Error> {
        let tasks_per_priority = args.tasks_per_priority.base10_parse::<u16>()?;

        let task_delay = args.task_delay.base10_parse::<usize>()?;

        let deadline_timings = args
            .deadline_timings
            .into_iter()
            .map(|lit| lit.base10_parse::<u32>())
            .collect::<Result<Vec<_>, _>>()?;

        if !(deadline_timings.windows(2).all(|w| w[0] < w[1])
            || deadline_timings.windows(2).all(|w| w[0] > w[1]))
        {
            return Err(
                "deadline_timings must be strictly ordered and contain no duplicates".into(),
            );
        }

        Ok(Settings {
            tasks_per_priority,
            task_delay,
            deadline_timings,
        })
    }
}

pub(crate) struct Settings {
    pub tasks_per_priority: u16,
    pub task_delay: usize,
    pub deadline_timings: Vec<u32>,
}

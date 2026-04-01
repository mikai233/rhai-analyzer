use crate::tests::completion::members::{
    completions_at, load_analysis_with_project, member_completion,
};
use rhai_project::{FunctionSpec, ProjectConfig, TypeSpec};

#[test]
fn completions_specialize_generic_host_method_details_from_receiver_types() {
    let (analysis, file_id, text) = load_analysis_with_project(
        r#"
            fn run() {
                /// @type Box<int>
                let boxed = unknown_box;
                boxed.
                next();
            }

            fn next() {}
        "#,
        ProjectConfig {
            types: [(
                "Box<T>".to_owned(),
                TypeSpec {
                    docs: None,
                    methods: [
                        (
                            "peek".to_owned(),
                            vec![FunctionSpec {
                                signature: "fun() -> T".to_owned(),
                                return_type: None,
                                docs: Some("Peek at the boxed value".to_owned()),
                            }],
                        ),
                        (
                            "unwrap_or".to_owned(),
                            vec![FunctionSpec {
                                signature: "fun(T) -> T".to_owned(),
                                return_type: None,
                                docs: Some("Return the boxed value or a fallback".to_owned()),
                            }],
                        ),
                    ]
                    .into_iter()
                    .collect(),
                },
            )]
            .into_iter()
            .collect(),
            ..ProjectConfig::default()
        },
    );

    let offset =
        u32::try_from(text.find("boxed.").expect("expected boxed member access") + "boxed.".len())
            .expect("offset");

    let completions = completions_at(&analysis, file_id, offset);
    let peek = member_completion(&completions, "peek");
    let unwrap_or = member_completion(&completions, "unwrap_or");

    assert_eq!(peek.detail.as_deref(), Some("fun() -> int"));
    assert_eq!(unwrap_or.detail.as_deref(), Some("fun(int) -> int"));
}

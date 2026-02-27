use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum QuestionType {
    Open,
    Single,
    Multi,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuizOption {
    pub id: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AnswerKey {
    Open { text: String },
    Single {
        #[serde(rename = "optionId")]
        option_id: String,
    },
    Multi {
        #[serde(rename = "optionIds")]
        option_ids: Vec<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Question {
    pub id: String,
    #[serde(rename = "type")]
    pub q_type: QuestionType,
    pub prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<QuizOption>>,
    pub answer: AnswerKey,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quiz {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub questions: Vec<Question>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SubmittedAnswer {
    Open { text: String },
    Single {
        #[serde(rename = "optionId")]
        option_id: String,
    },
    Multi {
        #[serde(rename = "optionIds")]
        option_ids: Vec<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StudentStats {
    pub nickname: String,
    pub correct: u32,
    pub wrong: u32,
}

impl StudentStats {
    pub fn correct_pct(&self) -> f64 {
        let total = self.correct + self.wrong;
        if total == 0 {
            0.0
        } else {
            (self.correct as f64) * 100.0 / (total as f64)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationIssue {
    pub field: String,
    pub issue: String,
}

pub fn validate_quiz(quiz: &Quiz) -> Result<(), Vec<ValidationIssue>> {
    let mut issues = Vec::new();
    if quiz.title.trim().is_empty() {
        issues.push(ValidationIssue {
            field: "title".into(),
            issue: "must not be empty".into(),
        });
    }
    if let Some(d) = &quiz.description {
        if d.trim().is_empty() {
            issues.push(ValidationIssue {
                field: "description".into(),
                issue: "must not be empty when present".into(),
            });
        }
    }
    if quiz.questions.is_empty() {
        issues.push(ValidationIssue {
            field: "questions".into(),
            issue: "must contain at least one question".into(),
        });
    }

    let mut question_ids = HashSet::new();
    for (i, q) in quiz.questions.iter().enumerate() {
        if q.id.trim().is_empty() {
            issues.push(ValidationIssue {
                field: format!("questions[{i}].id"),
                issue: "must not be empty".into(),
            });
        }
        if !question_ids.insert(q.id.clone()) {
            issues.push(ValidationIssue {
                field: format!("questions[{i}].id"),
                issue: "must be unique".into(),
            });
        }
        if q.prompt.trim().is_empty() {
            issues.push(ValidationIssue {
                field: format!("questions[{i}].prompt"),
                issue: "must not be empty".into(),
            });
        }

        match q.q_type {
            QuestionType::Open => {
                if q.options.is_some() {
                    issues.push(ValidationIssue {
                        field: format!("questions[{i}].options"),
                        issue: "must be absent for open question".into(),
                    });
                }
                match &q.answer {
                    AnswerKey::Open { text } => {
                        if text.trim().is_empty() {
                            issues.push(ValidationIssue {
                                field: format!("questions[{i}].answer.text"),
                                issue: "must not be empty".into(),
                            });
                        }
                    }
                    _ => issues.push(ValidationIssue {
                        field: format!("questions[{i}].answer"),
                        issue: "must match open format".into(),
                    }),
                }
            }
            QuestionType::Single | QuestionType::Multi => {
                let options = q.options.as_ref();
                if options.is_none() {
                    issues.push(ValidationIssue {
                        field: format!("questions[{i}].options"),
                        issue: "is required for single/multi".into(),
                    });
                }
                let mut map = HashMap::new();
                if let Some(opts) = options {
                    if opts.len() < 2 {
                        issues.push(ValidationIssue {
                            field: format!("questions[{i}].options"),
                            issue: "must contain at least 2 options".into(),
                        });
                    }
                    for (j, opt) in opts.iter().enumerate() {
                        if opt.id.trim().is_empty() || opt.text.trim().is_empty() {
                            issues.push(ValidationIssue {
                                field: format!("questions[{i}].options[{j}]"),
                                issue: "id/text must not be empty".into(),
                            });
                        }
                        if map.insert(opt.id.clone(), true).is_some() {
                            issues.push(ValidationIssue {
                                field: format!("questions[{i}].options[{j}].id"),
                                issue: "must be unique".into(),
                            });
                        }
                    }
                }
                match (&q.q_type, &q.answer) {
                    (QuestionType::Single, AnswerKey::Single { option_id }) => {
                        if option_id.trim().is_empty() {
                            issues.push(ValidationIssue {
                                field: format!("questions[{i}].answer.optionId"),
                                issue: "must not be empty".into(),
                            });
                        }
                        if let Some(opts) = options {
                            if !opts.iter().any(|o| o.id == *option_id) {
                                issues.push(ValidationIssue {
                                    field: format!("questions[{i}].answer.optionId"),
                                    issue: "must reference existing option id".into(),
                                });
                            }
                        }
                    }
                    (QuestionType::Multi, AnswerKey::Multi { option_ids }) => {
                        if option_ids.is_empty() {
                            issues.push(ValidationIssue {
                                field: format!("questions[{i}].answer.optionIds"),
                                issue: "must not be empty".into(),
                            });
                        }
                        let mut seen = HashSet::new();
                        for (k, id) in option_ids.iter().enumerate() {
                            if !seen.insert(id.clone()) {
                                issues.push(ValidationIssue {
                                    field: format!("questions[{i}].answer.optionIds[{k}]"),
                                    issue: "must be unique".into(),
                                });
                            }
                            if let Some(opts) = options {
                                if !opts.iter().any(|o| o.id == *id) {
                                    issues.push(ValidationIssue {
                                        field: format!("questions[{i}].answer.optionIds[{k}]"),
                                        issue: "must reference existing option id".into(),
                                    });
                                }
                            }
                        }
                    }
                    _ => issues.push(ValidationIssue {
                        field: format!("questions[{i}].answer"),
                        issue: "must match question type".into(),
                    }),
                }
            }
        }
    }

    if issues.is_empty() {
        Ok(())
    } else {
        Err(issues)
    }
}

pub fn score_answer(question: &Question, submitted: &SubmittedAnswer) -> bool {
    match (&question.answer, submitted) {
        (AnswerKey::Open { text }, SubmittedAnswer::Open { text: value }) => {
            text.trim().eq_ignore_ascii_case(value.trim())
        }
        (AnswerKey::Single { option_id }, SubmittedAnswer::Single { option_id: value }) => {
            option_id == value
        }
        (AnswerKey::Multi { option_ids }, SubmittedAnswer::Multi { option_ids: value }) => {
            let expected: HashSet<_> = option_ids.iter().collect();
            let actual: HashSet<_> = value.iter().collect();
            expected == actual
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_quiz() -> Quiz {
        Quiz {
            title: "Test".into(),
            description: Some("Desc".into()),
            questions: vec![
                Question {
                    id: "q1".into(),
                    q_type: QuestionType::Open,
                    prompt: "2+2".into(),
                    options: None,
                    answer: AnswerKey::Open { text: "4".into() },
                },
                Question {
                    id: "q2".into(),
                    q_type: QuestionType::Single,
                    prompt: "Capital".into(),
                    options: Some(vec![
                        QuizOption { id: "o1".into(), text: "Paris".into() },
                        QuizOption { id: "o2".into(), text: "Rome".into() },
                    ]),
                    answer: AnswerKey::Single { option_id: "o1".into() },
                },
                Question {
                    id: "q3".into(),
                    q_type: QuestionType::Multi,
                    prompt: "Even".into(),
                    options: Some(vec![
                        QuizOption { id: "o1".into(), text: "2".into() },
                        QuizOption { id: "o2".into(), text: "3".into() },
                        QuizOption { id: "o3".into(), text: "4".into() },
                    ]),
                    answer: AnswerKey::Multi { option_ids: vec!["o1".into(), "o3".into()] },
                },
            ],
        }
    }

    #[test]
    fn validate_quiz_ok() {
        let quiz = sample_quiz();
        assert!(validate_quiz(&quiz).is_ok());
    }

    #[test]
    fn validate_quiz_negative() {
        let mut quiz = sample_quiz();
        quiz.questions[0].options = Some(vec![]);
        quiz.questions[1].id = "q1".into();
        let result = validate_quiz(&quiz);
        assert!(result.is_err());
        let issues = result.err().unwrap();
        assert!(issues.iter().any(|i| i.issue.contains("unique")));
    }

    #[test]
    fn scoring_open_single_multi() {
        let quiz = sample_quiz();
        assert!(score_answer(
            &quiz.questions[0],
            &SubmittedAnswer::Open { text: " 4 ".into() }
        ));
        assert!(score_answer(
            &quiz.questions[1],
            &SubmittedAnswer::Single { option_id: "o1".into() }
        ));
        assert!(score_answer(
            &quiz.questions[2],
            &SubmittedAnswer::Multi { option_ids: vec!["o3".into(), "o1".into()] }
        ));
        assert!(!score_answer(
            &quiz.questions[2],
            &SubmittedAnswer::Multi { option_ids: vec!["o1".into()] }
        ));
    }

    #[test]
    fn student_stats_pct() {
        let s = StudentStats {
            nickname: "N".into(),
            correct: 3,
            wrong: 1,
        };
        assert_eq!(s.correct_pct(), 75.0);
    }
}

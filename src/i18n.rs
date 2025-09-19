pub trait Language {
    fn file_tree_title(&self) -> &'static str;
    fn editor_title(&self) -> &'static str;
    fn instructions_title(&self) -> &'static str;
    fn quit_instruction(&self) -> &'static str;
    fn counter_instruction(&self) -> &'static str;
    fn lang_toggle_instruction(&self) -> &'static str;
    fn counter_title(&self) -> &'static str;
}

pub struct English;
impl Language for English {
    fn file_tree_title(&self) -> &'static str { "File Tree" }
    fn editor_title(&self) -> &'static str { "Editor" }
    fn instructions_title(&self) -> &'static str { "Instructions" }
    fn quit_instruction(&self) -> &'static str { "Press 'q' to quit." }
    fn counter_instruction(&self) -> &'static str { "Press '←'/'→' to change counter." }
    fn lang_toggle_instruction(&self) -> &'static str { "Press 'l' to switch language." }
    fn counter_title(&self) -> &'static str { "Counter" }
}

pub struct SimplifiedChinese;
impl Language for SimplifiedChinese {
    fn file_tree_title(&self) -> &'static str { "文件树" }
    fn editor_title(&self) -> &'static str { "编辑器" }
    fn instructions_title(&self) -> &'static str { "操作说明" }
    fn quit_instruction(&self) -> &'static str { "按 'q' 退出。" }
    fn counter_instruction(&self) -> &'static str { "按 '←'/'→' 改变计数器。" }
    fn lang_toggle_instruction(&self) -> &'static str { "按 'l' 切换语言。" }
    fn counter_title(&self) -> &'static str { "计数器" }
}

pub struct TraditionalChinese;
impl Language for TraditionalChinese {
    fn file_tree_title(&self) -> &'static str { "檔案總管" }
    fn editor_title(&self) -> &'static str { "編輯器" }
    fn instructions_title(&self) -> &'static str { "操作說明" }
    fn quit_instruction(&self) -> &'static str { "按 'q' 退出。" }
    fn counter_instruction(&self) -> &'static str { "按 '←'/'→' 變更計數器。" }
    fn lang_toggle_instruction(&self) -> &'static str { "按 'l' 切換語言。" }
    fn counter_title(&self) -> &'static str { "計數器" }
}
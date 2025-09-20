pub trait Language {
    // Titles
    fn editor_title(&self) -> &'static str;
    fn file_tree_title(&self) -> &'static str;

    // Header
    fn header_file(&self) -> &'static str;
    fn header_edit(&self) -> &'static str;
    fn header_view(&self) -> &'static str;
    fn header_go(&self) -> &'static str;
    fn header_run(&self) -> &'static str;
    fn header_terminal(&self) -> &'static str;
    fn header_help(&self) -> &'static str;

    // Footer
    fn footer_no_file(&self) -> &'static str;
    fn footer_line(&self) -> &'static str;
    fn footer_col(&self) -> &'static str;
    fn footer_lang_toggle(&self) -> &'static str;
}

pub struct English;
impl Language for English {
    fn editor_title(&self) -> &'static str { "Editor" }
    fn file_tree_title(&self) -> &'static str { " Explorer " }
    fn header_file(&self) -> &'static str { "File" }
    fn header_edit(&self) -> &'static str { "Edit" }
    fn header_view(&self) -> &'static str { "View" }
    fn header_go(&self) -> &'static str { "Go" }
    fn header_run(&self) -> &'static str { "Run" }
    fn header_terminal(&self) -> &'static str { "Terminal" }
    fn header_help(&self) -> &'static str { "Help" }
    fn footer_no_file(&self) -> &'static str { "[No Name]" }
    fn footer_line(&self) -> &'static str { "Ln" }
    fn footer_col(&self) -> &'static str { "Col" }
    fn footer_lang_toggle(&self) -> &'static str { "Press 'l' to switch language" }
}

pub struct SimplifiedChinese;
impl Language for SimplifiedChinese {
    fn editor_title(&self) -> &'static str { "编辑器" }
    fn file_tree_title(&self) -> &'static str { " 文件浏览器 " }
    fn header_file(&self) -> &'static str { "文件" }
    fn header_edit(&self) -> &'static str { "编辑" }
    fn header_view(&self) -> &'static str { "视图" }
    fn header_go(&self) -> &'static str { "转到" }
    fn header_run(&self) -> &'static str { "运行" }
    fn header_terminal(&self) -> &'static str { "终端" }
    fn header_help(&self) -> &'static str { "帮助" }
    fn footer_no_file(&self) -> &'static str { "[无名称]" }
    fn footer_line(&self) -> &'static str { "行" }
    fn footer_col(&self) -> &'static str { "列" }
    fn footer_lang_toggle(&self) -> &'static str { "按 'l' 切换语言" }
}

pub struct TraditionalChinese;
impl Language for TraditionalChinese {
    fn editor_title(&self) -> &'static str { "編輯器" }
    fn file_tree_title(&self) -> &'static str { " 檔案總管 " }
    fn header_file(&self) -> &'static str { "檔案" }
    fn header_edit(&self) -> &'static str { "編輯" }
    fn header_view(&self) -> &'static str { "檢視" }
    fn header_go(&self) -> &'static str { "前往" }
    fn header_run(&self) -> &'static str { "執行" }
    fn header_terminal(&self) -> &'static str { "終端機" }
    fn header_help(&self) -> &'static str { "說明" }
    fn footer_no_file(&self) -> &'static str { "[未命名]" }
    fn footer_line(&self) -> &'static str { "行" }
    fn footer_col(&self) -> &'static str { "列" }
    fn footer_lang_toggle(&self) -> &'static str { "按 'l' 切換語言" }
}
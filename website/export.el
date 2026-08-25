;; Export website/index.org to website/index.html (ox-html).
;; Usage: emacs --batch -l website/export.el
(require 'ox-html)

(setq org-export-with-toc nil
      org-export-with-section-numbers nil
      org-export-with-author nil
      org-export-with-timestamps nil
      org-export-time-stamp-file nil
      org-export-with-sub-superscripts nil
      org-html-head-include-default-style nil
      org-html-head-include-scripts nil
      org-html-validation-link nil
      org-html-creator-string nil
      org-html-preamble nil
      org-html-postamble nil
      org-html-doctype "html5"
      org-html-html5-fancy t
      org-html-htmlize-output-type nil
      org-html-container-element "section"
      org-html-divs
      '((preamble "div" "preamble")
        (content "div" "content")
        (postamble "div" "postamble"))
      org-html-text-markup-alist
      '((bold . "<strong>%s</strong>")
        (code . "<code>%s</code>")
        (italic . "<em>%s</em>")
        (strike-through . "<del>%s</del>")
        (underline . "<span class=\"underline\">%s</span>")
        (verbatim . "<code>%s</code>")))

(let ((dir (file-name-directory (or load-file-name buffer-file-name))))
  (find-file (expand-file-name "index.org" dir))
  (org-html-export-to-html))

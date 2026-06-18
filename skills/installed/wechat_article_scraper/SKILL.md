---
name: "wechat_article_scraper"
description: "Fetches full content from WeChat Official Account articles (mp.weixin.qq.com), handling lazy-loaded images and converting to clean Markdown. Invoke when user wants to extract, read, or summarize WeChat public account articles."
---

# 🕵️‍♂️ WeChat Article Scraper

## 🎯 Overview
A specialized skill for extracting high-fidelity content from WeChat Official Account articles. Standard web fetching tools often fail to capture images due to WeChat's `data-src` lazy-loading mechanism. This skill enforces a specific Python-based extraction workflow to guarantee content completeness.

## 💡 When to Use
- Fetch and read WeChat articles offline
- Extract article content for summarization
- Save WeChat articles as Markdown files
- Batch collect content from WeChat public accounts

## ⚔️ Execution Rules
1. **Lazy Loading Handling**: WeChat images are stored in `data-src`. Must use custom script to extract them.
2. **Full Content**: Capture the entire article body (`#js_content`), including text and images.
3. **Markdown Output**: Convert content to clean Markdown with standard image syntax `![image](url)`.

## 🛠️ Usage

### Step 1: Provide the WeChat URL
User provides a WeChat article URL (e.g., `https://mp.weixin.qq.com/s/xxxxx`)

### Step 2: Run the Scraper
```python
# Save as fetch_wx.py and run
import requests
import re
import sys

def fetch_wechat(url, output_path):
    headers = {
        "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36"
    }
    
    response = requests.get(url, headers=headers)
    html = response.text
    
    # Extract Title
    title_match = re.search(r'property="og:title" content="([^"]+)"', html)
    title = title_match.group(1) if title_match else "WeChat_Article"
    
    # Extract Main Content Div
    content_match = re.search(r'<div class="rich_media_content.*?id="js_content".*?>(.*?)</div>', html, re.DOTALL)
    if not content_match:
        return "Error: Could not find content div"
    
    content = content_match.group(1)
    
    # Replace data-src images with Markdown format
    def replace_img(match):
        src_match = re.search(r'data-src=["\'](.*?)["\']', match.group(0))
        return f'\n\n![image]({src_match.group(1)})\n\n' if src_match else ''
    
    content = re.sub(r'<img[^>]+>', replace_img, content)
    content = re.sub(r'<br\s*/?>', '\n', content)
    content = re.sub(r'</p>', '\n\n', content)
    content = re.sub(r'<[^>]+>', '', content)
    
    lines = [line.strip() for line in content.split('\n') if line.strip()]
    return f"# {title}\n\n" + "\n\n".join(lines)

# Usage
result = fetch_wechat("YOUR_WECHAT_URL", "output.md")
print(result)
```

### Step 3: Get the Markdown Output
The article content is extracted and saved as a clean Markdown file.

## 📊 Example Output

```markdown
# 公众号文章标题

![image](https://mmbiz.qpic.cn/...)

这是文章正文内容...

![image](https://mmbiz.qpic.cn/...)
```

## 🎯 Supported Features
- ✅ Article title extraction
- ✅ Full text content extraction
- ✅ Lazy-loaded image handling (`data-src` → `![image]`)
- ✅ Markdown format conversion
- ✅ Batch processing support

## ⚠️ Limitations
- Some articles may require authentication
- Images may have expiration dates
- Articles behind paywall cannot be scraped

## 🚀 Trigger Keywords
- "抓取微信公众号"
- "获取微信文章"
- "WeChat article"
- "微信公众号内容"

import os

def fix_mojibake(text):
    try:
        # The logic: Mojibake happened when UTF-8 bytes were read as GBK and saved back as UTF-8.
        # To reverse: Encode the mojibake string as GBK to get original UTF-8 bytes, then decode as UTF-8.
        return text.encode('gbk').decode('utf-8')
    except (UnicodeEncodeError, UnicodeDecodeError):
        return text

def process_file(file_path):
    try:
        with open(file_path, 'r', encoding='utf-8') as f:
            content = f.read()
        
        # We only want to fix if it looks like mojibake. 
        # Common signs: '璇', '姝', '鐧'
        if any(c in content for c in '璇姝鐧'):
            fixed_content = fix_mojibake(content)
            if fixed_content != content:
                with open(file_path, 'w', encoding='utf-8') as f:
                    f.write(fixed_content)
                print(f"Fixed: {file_path}")
                return True
    except Exception as e:
        print(f"Error processing {file_path}: {e}")
    return False

# Files to fix explicitly
targets = [
    r'.github\pull_request_template.md',
    r'cloud\dashboard\login.js'
]

for t in targets:
    process_file(t)

# Also scan a few key directories
for root, dirs, files in os.walk('.'):
    if any(x in root for x in ['.git', '.venv', 'node_modules', 'models', 'datasets']):
        continue
    for file in files:
        if file.endswith(('.py', '.js', '.md', '.html', '.css')):
            process_file(os.path.join(root, file))

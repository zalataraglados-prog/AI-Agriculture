import os

def fix_mojibake(text):
    try:
        # Step 1: Try the most common Mojibake (UTF-8 bytes read as GBK)
        return text.encode('gbk').decode('utf-8')
    except:
        try:
            # Step 2: Try another common one (GBK bytes read as UTF-8 - though less likely here)
            return text.encode('utf-8').decode('gbk')
        except:
            return text

def force_fix_file(file_path):
    try:
        with open(file_path, 'r', encoding='utf-8') as f:
            content = f.read()
        
        fixed_content = fix_mojibake(content)
        if fixed_content != content:
            with open(file_path, 'w', encoding='utf-8') as f:
                f.write(fixed_content)
            print(f"Forced Fixed: {file_path}")
            return True
    except Exception as e:
        print(f"Error: {e}")
    return False

# Specifically target the PR template
force_fix_file(r'.github\pull_request_template.md')

# Scan more characters
for root, dirs, files in os.walk('.'):
    if any(x in root for x in ['.git', '.venv', 'node_modules']): continue
    for file in files:
        if file.endswith(('.py', '.js', '.md', '.html')):
            path = os.path.join(root, file)
            with open(path, 'r', encoding='utf-8', errors='ignore') as f:
                c = f.read()
            if any(char in c for char in '鑳鏀鍙'): # Add characters found in PR template
                force_fix_file(path)

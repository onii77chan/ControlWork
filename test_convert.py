import subprocess
import os
import sys

# Create a dummy docx using python-docx if needed, but since we don't have it installed we can just test if the imports work in convert.py
subprocess.run([sys.executable, "-c", "import mammoth; import xhtml2pdf; from pdf2docx import Converter"], check=True)
print("Imports work.")

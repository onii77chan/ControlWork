#!/usr/bin/env python3
import sys
import os
import argparse
import subprocess
from pdf2docx import Converter

def convert_pdf_to_docx(input_path, output_path):
    cv = Converter(input_path)
    cv.convert(output_path)
    cv.close()

def convert_docx_to_pdf(input_path, output_path):
    try:
        import mammoth
        from xhtml2pdf import pisa

        # 1. Convert DOCX to HTML using mammoth
        with open(input_path, "rb") as docx_file:
            result = mammoth.convert_to_html(docx_file)
            html = result.value # The generated HTML

            # Wrap in basic HTML structure to ensure proper encoding and display
            full_html = f"""
            <html>
            <head>
                <meta charset="utf-8">
                <style>
                    body {{ font-family: sans-serif; }}
                </style>
            </head>
            <body>
                {html}
            </body>
            </html>
            """

            # 2. Convert HTML to PDF using xhtml2pdf
            with open(output_path, "w+b") as pdf_file:
                pisa_status = pisa.CreatePDF(full_html, dest=pdf_file)

            if pisa_status.err:
                raise Exception(f"xhtml2pdf error: {pisa_status.err}")

    except Exception as e:
        print(f"docx to pdf conversion failed: {e}")
        # Fallback to pandoc if mammoth/xhtml2pdf fails or is missing
        print("Trying fallback to pandoc...")
        convert_with_pandoc(input_path, output_path)

def convert_with_pandoc(input_path, output_path):
    # Use pandoc for various document conversions (e.g., md to docx, html to md, etc.)
    subprocess.run(["pandoc", input_path, "-o", output_path], check=True)

def main():
    parser = argparse.ArgumentParser(description="Universal document converter for Telegram Bot.")
    parser.add_argument("input", help="Path to the input file")
    parser.add_argument("output", help="Path to the output file")
    parser.add_argument("target_format", help="Target format extension (e.g., pdf, docx)")

    args = parser.parse_args()

    input_path = args.input
    output_path = args.output
    target_format = args.target_format.lower()

    if not os.path.exists(input_path):
        print(f"Error: Input file {input_path} does not exist.")
        sys.exit(1)

    input_ext = os.path.splitext(input_path)[1].lower().strip('.')

    try:
        if input_ext == 'pdf' and target_format == 'docx':
            convert_pdf_to_docx(input_path, output_path)
        elif input_ext == 'docx' and target_format == 'pdf':
            convert_docx_to_pdf(input_path, output_path)
        else:
            # Fallback to pandoc for other conversions
            convert_with_pandoc(input_path, output_path)

        print(f"Successfully converted {input_path} to {output_path}")
        sys.exit(0)
    except Exception as e:
        print(f"Conversion failed: {e}")
        sys.exit(1)

if __name__ == "__main__":
    main()

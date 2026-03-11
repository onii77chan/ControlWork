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
        from docx2pdf import convert
        convert(input_path, output_path)
    except Exception as e:
        # docx2pdf requires MS Word to be installed on Windows/Mac,
        # for a truly headless/Linux environment, LibreOffice is usually better.
        # Fallback to soffice (LibreOffice) if available:
        print(f"docx2pdf failed: {e}. Trying LibreOffice...")
        # Get the directory of output_path
        out_dir = os.path.dirname(output_path)
        if not out_dir:
            out_dir = "."

        # Note: LibreOffice always writes to a file with the same name but .pdf extension in the outdir.
        # We will rename it afterwards if necessary.
        subprocess.run(["soffice", "--headless", "--convert-to", "pdf", "--outdir", out_dir, input_path], check=True)

        # The expected output name from soffice:
        base_name = os.path.splitext(os.path.basename(input_path))[0]
        expected_out = os.path.join(out_dir, base_name + ".pdf")

        # If output_path is different from what soffice generated, rename it.
        if os.path.abspath(expected_out) != os.path.abspath(output_path):
             os.rename(expected_out, output_path)

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

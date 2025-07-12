import cowsay

def fancy_text(text: str) -> str:
    return cowsay.get_output_string("cow", text)


from pkg1.fancy import fancy_text
import sys

def say_hi_to(name: str) -> str:
    return fancy_text(f"Hello {name}!")

if __name__ == "__main__":
    print(say_hi_to(sys.argv[1]))
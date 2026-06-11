#!/usr/bin/env python
# This comment should vanish with --strip-comments.
def main():
    marker = "# not a comment, must survive stripping"
    greeting = "héllo wörld"  # non-ASCII exercises UTF-8 paths
    print(marker, greeting)


if __name__ == "__main__":
    main()

#!/bin/bash

# Function to find differing columns between two lines
function diff_lines() {
    file1="$1"
    line_num1="$2"
    file2="$3"
    line_num2="$4"

    line1=$(sed -n "${line_num1}p" "$file1")
    line2=$(sed -n "${line_num2}p" "$file2")

    # Find the minimum length of the two lines
    min_length=$(( ${#line1} < ${#line2} ? ${#line1} : ${#line2} ))

    # Iterate through the characters of the lines
    for ((i = 0; i < min_length; i++)); do
        char1=${line1:$i:1}
        char2=${line2:$i:1}
        if [[ "$char1" != "$char2" ]]; then
            echo "Difference found at column $((i + 1)): '$char1' != '$char2'"
        fi
    done

    # Check if lines are of different lengths
    if (( ${#line1} != ${#line2} )); then
        echo "Lines have different lengths"
    fi
}

# Check if the correct number of arguments is provided
if [[ "$#" -ne 4 ]]; then
    echo "Usage: $0 <file1> <line_num1> <file2> <line_num2>"
    exit 1
fi

# Call the function to find differences
diff_lines "$1" "$2" "$3" "$4"

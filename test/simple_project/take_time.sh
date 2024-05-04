for i in 1 2 3 4 5 ; do
    sleep 1;
    echo -e \\033[31mHELLO\\033[39m\\033[49m ${i}!;
done

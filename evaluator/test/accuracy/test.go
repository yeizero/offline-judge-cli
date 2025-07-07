package main

import (
    "bufio"
    "fmt"
    "os"
)

func main() {
    scanner := bufio.NewScanner(os.Stdin)
    for scanner.Scan() {
        var a, b int
        _, err := fmt.Sscanf(scanner.Text(), "%d %d", &a, &b)
        if err == nil {
            fmt.Println(a + b)
        }
    }
}
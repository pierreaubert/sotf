import Darwin

@main
struct SotFHALTestsRunner {
    static func main() {
        if !HALDriverTests.runAllTests() {
            exit(EXIT_FAILURE)
        }
    }
}

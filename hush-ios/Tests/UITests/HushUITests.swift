import XCTest

/// User journeys against the Debug-only fake boundaries in `UITestHarness`. Placeholder data only.
/// These prove view behavior. They do not prove a live Bitwarden connection.
@MainActor final class HushUITests: XCTestCase {
    private lazy var app = XCUIApplication()

    private func launch(_ scenario: String) {
        continueAfterFailure = false
        app.launchEnvironment["HUSH_UI_TEST_SCENARIO"] = scenario
        app.launch()
    }

    private func element(_ id: String) -> XCUIElement {
        app.descendants(matching: .any).matching(identifier: id).firstMatch
    }

    private func rows() -> XCUIElementQuery {
        app.descendants(matching: .any).matching(identifier: "secret-row")
    }

    private func unlock() {
        let unlock = app.buttons["unlock-button"]
        XCTAssertTrue(unlock.waitForExistence(timeout: 5))
        unlock.tap()
        XCTAssertTrue(app.buttons["add-button"].waitForExistence(timeout: 5))
    }

    /// Connect, search, and open two secrets that share a key. Each opens its own masked detail.
    func testConnectSearchAndOpenSecretsThatShareAKey() {
        launch("fresh")
        let token = app.secureTextFields["token-field"]
        XCTAssertTrue(token.waitForExistence(timeout: 5))
        token.tap()
        token.typeText("placeholder-token")
        app.buttons["connect-button"].tap()
        XCTAssertTrue(app.buttons["add-button"].waitForExistence(timeout: 5))

        let search = app.searchFields.firstMatch
        search.tap()
        search.typeText("example_token")
        XCTAssertEqual(element("list-status").label, "3 secrets")

        rows().element(boundBy: 0).tap()
        XCTAssertTrue(element("detail-value-masked").waitForExistence(timeout: 5))
        XCTAssertFalse(app.staticTexts["placeholder-one"].exists)
        XCTAssertFalse(app.staticTexts["placeholder-two"].exists)
        let firstID = element("detail-id").label

        element("reveal-button").tap()
        XCTAssertTrue(element("detail-value").waitForExistence(timeout: 2))
        let firstValue = element("detail-value").label

        if app.navigationBars.buttons.element(boundBy: 0).exists, !app.buttons["add-button"].isHittable {
            app.navigationBars.buttons.element(boundBy: 0).tap()
        }
        rows().element(boundBy: 1).tap()
        XCTAssertTrue(element("detail-value-masked").waitForExistence(timeout: 5))
        XCTAssertNotEqual(element("detail-id").label, firstID)
        element("reveal-button").tap()
        XCTAssertTrue(element("detail-value").waitForExistence(timeout: 2))
        XCTAssertNotEqual(element("detail-value").label, firstValue)
    }

    /// Going to the background locks, and the revealed value is gone when the app returns.
    func testBackgroundLocksAndClearsTheOpenSecret() {
        launch("saved")
        unlock()
        rows().element(boundBy: 0).tap()
        XCTAssertTrue(element("reveal-button").waitForExistence(timeout: 5))
        element("reveal-button").tap()
        XCTAssertTrue(element("detail-value").waitForExistence(timeout: 2))

        XCUIDevice.shared.press(.home)
        app.activate()

        XCTAssertTrue(app.buttons["unlock-button"].waitForExistence(timeout: 5))
        XCTAssertFalse(element("detail-value").exists)
        XCTAssertFalse(element("detail-value-masked").exists)
    }

    /// Empty save marks the field and keeps the draft. A good save opens the new secret, masked.
    func testAddValidatesThenOpensTheNewSecret() {
        launch("saved")
        unlock()
        app.buttons["add-button"].tap()
        let save = app.buttons["draft-save"]
        XCTAssertTrue(save.waitForExistence(timeout: 5))
        save.tap()
        XCTAssertTrue(element("draft-error").waitForExistence(timeout: 2))

        chooseProject("Example Beta")
        let key = app.textFields["draft-key"]
        key.tap()
        key.typeText("NEW_PLACEHOLDER")
        save.tap()
        XCTAssertEqual(element("draft-error").label, "Enter a value.")
        XCTAssertEqual(app.textFields["draft-key"].value as? String, "NEW_PLACEHOLDER")

        let value = app.secureTextFields["draft-value"]
        value.tap()
        value.typeText("placeholder-new")
        save.tap()

        XCTAssertTrue(element("detail-value-masked").waitForExistence(timeout: 5))
        XCTAssertTrue(element("detail-key").label.contains("NEW_PLACEHOLDER"))
        XCTAssertFalse(app.staticTexts["placeholder-new"].exists)
    }

    /// A lost create reply is never retried. The user refreshes, sees the candidate, and marks it reviewed.
    func testLostCreateReplyNeedsReviewInsteadOfRetry() {
        launch("lostCreateReply")
        unlock()
        app.buttons["add-button"].tap()
        chooseProject("Example Alpha")
        let key = app.textFields["draft-key"]
        key.tap()
        key.typeText("LOST_PLACEHOLDER")
        let value = app.secureTextFields["draft-value"]
        value.tap()
        value.typeText("placeholder-lost")
        app.buttons["draft-save"].tap()

        XCTAssertTrue(element("pending-summary").waitForExistence(timeout: 5))
        XCTAssertFalse(app.buttons["add-button"].isEnabled)
        XCTAssertFalse(element("pending-acknowledge").exists)

        element("pending-refresh").tap()
        XCTAssertTrue(element("pending-acknowledge").waitForExistence(timeout: 5))
        XCTAssertEqual(app.descendants(matching: .any).matching(identifier: "pending-candidate").count, 1)

        element("pending-acknowledge").tap()
        XCTAssertFalse(element("pending-summary").waitForExistence(timeout: 1))
        XCTAssertTrue(app.buttons["add-button"].isEnabled)
    }

    /// Settings shows the saved region, opens the bundled notices, and Lock now returns to the lock screen.
    func testSettingsShowsRegionNoticesAndLocks() {
        launch("fresh")
        let token = app.secureTextFields["token-field"]
        XCTAssertTrue(token.waitForExistence(timeout: 5))
        token.tap()
        token.typeText("placeholder-token")
        app.buttons["connect-button"].tap()
        XCTAssertTrue(app.buttons["settings-button"].waitForExistence(timeout: 5))

        XCTAssertEqual(element("list-status").label, "4 secrets")
        attachScreenshot("catalog-count")
        app.buttons["settings-button"].tap()
        XCTAssertTrue(element("settings-region").waitForExistence(timeout: 5))
        XCTAssertTrue(element("settings-region").label.contains("United States"))

        XCTAssertEqual(element("settings-notices").label, "Third-party notices")
        attachScreenshot("settings-label")
        element("settings-notices").tap()
        let notices = app.textViews["notices-text"]
        XCTAssertTrue(notices.waitForExistence(timeout: 10))
        let firstPage = notices.value as? String ?? ""
        XCTAssertTrue(firstPage.hasPrefix("Hush third-party notices"))
        XCTAssertLessThanOrEqual(firstPage.count, 8_192)
        XCTAssertTrue(firstPage.contains("Apache License"))
        XCTAssertFalse(app.buttons["notices-previous"].isEnabled)
        attachScreenshot("notices-first-page")

        app.buttons["notices-next"].tap()
        let secondPage = notices.value as? String ?? ""
        XCTAssertFalse(secondPage.isEmpty)
        XCTAssertLessThanOrEqual(secondPage.count, 8_192)
        XCTAssertNotEqual(secondPage, firstPage)
        XCTAssertTrue(app.buttons["notices-previous"].isEnabled)
        app.buttons["notices-previous"].tap()
        XCTAssertEqual(notices.value as? String, firstPage)
        app.navigationBars["Third-party notices"].buttons.firstMatch.tap()
        XCTAssertTrue(element("settings-region").waitForExistence(timeout: 5))

        element("settings-lock").tap()
        XCTAssertTrue(app.buttons["unlock-button"].waitForExistence(timeout: 5))
    }

    private func attachScreenshot(_ name: String) {
        let attachment = XCTAttachment(screenshot: app.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }

    private func chooseProject(_ name: String) {
        let picker = app.buttons["draft-project"]
        XCTAssertTrue(picker.waitForExistence(timeout: 5))
        picker.tap()
        let option = app.buttons[name]
        XCTAssertTrue(option.waitForExistence(timeout: 2))
        option.tap()
    }
}
